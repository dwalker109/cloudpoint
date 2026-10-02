/// CURL wrapper around a patched curl-sys (libcurl).
///
/// It is possible to use libraries like ureq and minreq on this hardware target,
/// but it has many problems, namely inability to use HTTPS or timeouts.
/// A CURL based option is available from devkitpro so we use it. This is a basic
/// wrapper around it.
///
/// Requires 3ds-zlib, 3ds-mbedtls, 3ds-curl be installed on the host system.
use curl_sys::*;
use libc::c_void;
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex};
use std::{io::Error as IoError, ptr};

#[derive(Debug)]
pub struct Response {
    pub status: u32,
    pub headers: Vec<String>,
    pub body: Vec<u8>,
}

struct Handle(*mut CURL);

// SAFETY: an easy handle may move between threads provided it is never used
// from two at once. It is only reachable through the Mutex in CurlHttpClient.
unsafe impl Send for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { curl_easy_cleanup(self.0) };
    }
}

#[derive(Clone)]
pub struct CurlHttpClient(Arc<Mutex<Handle>>);

fn check(code: CURLcode) -> Result<(), IoError> {
    if code == CURLE_OK {
        Ok(())
    } else {
        let msg = unsafe {
            let raw_msg = curl_sys::curl_easy_strerror(code);
            CStr::from_ptr(raw_msg).to_string_lossy()
        };

        Err(IoError::other(format!("libcurl error {code}: {msg}")))
    }
}

fn build_headers(extra: &[(&str, &str)]) -> Result<Slist, IoError> {
    let mut slist = Slist::new();

    for (k, v) in extra {
        slist.append(&format!("{k}: {v}"))?;
    }

    Ok(slist)
}

impl Handle {
    fn set_long(&self, opt: CURLoption, val: libc::c_long) -> Result<(), IoError> {
        check(unsafe { curl_easy_setopt(self.0, opt, val) })
            .map_err(|e| IoError::other(format!("{e} (CURLoption {opt})")))
    }

    fn set_ptr(&self, opt: CURLoption, val: *const c_void) -> Result<(), IoError> {
        check(unsafe { curl_easy_setopt(self.0, opt, val) })
            .map_err(|e| IoError::other(format!("{e} (CURLoption {opt})")))
    }

    fn set_off_t(&self, opt: CURLoption, val: curl_off_t) -> Result<(), IoError> {
        check(unsafe { curl_easy_setopt(self.0, opt, val) })
            .map_err(|e| IoError::other(format!("{e} (CURLoption {opt})")))
    }

    fn response_code(&self) -> u32 {
        let mut code: libc::c_long = 0;
        unsafe { curl_easy_getinfo(self.0, CURLINFO_RESPONSE_CODE, &mut code) };

        code as u32
    }

    fn reset_request(&self) -> Result<(), IoError> {
        self.set_long(CURLOPT_HTTPGET, 0)?;
        self.set_long(CURLOPT_NOBODY, 0)?;
        self.set_long(CURLOPT_UPLOAD, 0)?;
        self.set_long(CURLOPT_POST, 0)?;
        self.set_ptr(CURLOPT_URL, ptr::null())?;
        self.set_ptr(CURLOPT_CUSTOMREQUEST, ptr::null())?;
        self.set_ptr(CURLOPT_READFUNCTION, ptr::null())?;
        self.set_ptr(CURLOPT_READDATA, ptr::null())?;
        self.set_ptr(CURLOPT_HTTPHEADER, ptr::null())?;
        self.set_off_t(CURLOPT_INFILESIZE_LARGE, -1)?;
        Ok(())
    }

    fn perform(&self, headers: &mut Vec<String>, body: &mut Vec<u8>) -> Result<(), IoError> {
        self.set_ptr(CURLOPT_WRITEDATA, body as *mut Vec<u8> as *mut c_void)?;
        self.set_ptr(
            CURLOPT_HEADERDATA,
            headers as *mut Vec<String> as *mut c_void,
        )?;
        check(unsafe { curl_easy_perform(self.0) })
    }
}

impl CurlHttpClient {
    pub fn new(app_ver: &str) -> Result<Self, IoError> {
        let raw = unsafe { curl_easy_init() };
        if raw.is_null() {
            return Err(IoError::other("libcurl curl_easy_init failed"));
        }

        // Wrap immediately so the handle is freed if anything below fails.
        let handle = Handle(raw);

        #[cfg(target_os = "horizon")]
        {
            let cainfo_c = CString::new("romfs:/cacert.pem")?;
            handle.set_ptr(CURLOPT_CAINFO, cainfo_c.as_ptr() as *const c_void)?;
        }

        let ua_c = CString::new(format!("Cloudpoint/{app_ver}"))?;
        handle.set_ptr(CURLOPT_USERAGENT, ua_c.as_ptr() as *const c_void)?;

        handle.set_long(CURLOPT_TIMEOUT, 30)?;
        handle.set_long(CURLOPT_CONNECTTIMEOUT, 10)?;
        handle.set_long(CURLOPT_FOLLOWLOCATION, 1)?;
        handle.set_long(CURLOPT_MAXREDIRS, 5)?;
        handle.set_ptr(CURLOPT_WRITEFUNCTION, callbacks::write_cb as *const c_void)?;
        handle.set_ptr(
            CURLOPT_HEADERFUNCTION,
            callbacks::header_cb as *const c_void,
        )?;

        Ok(Self(Arc::new(Mutex::new(handle))))
    }

    pub fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, IoError> {
        log::debug!("performing libcurl GET to {url}");

        let h = self.0.lock().expect("lock on curl easy handle");

        h.reset_request()?;

        let url_c = CString::new(url)?;
        h.set_ptr(CURLOPT_URL, url_c.as_ptr() as *const c_void)?;
        h.set_long(CURLOPT_HTTPGET, 1)?;

        let slist = build_headers(headers)?;
        h.set_ptr(CURLOPT_HTTPHEADER, slist.as_ptr() as *const c_void)?;

        let mut headers = Vec::new();
        let mut body = Vec::new();
        h.perform(&mut headers, &mut body)?;

        Ok(Response {
            status: h.response_code(),
            headers,
            body,
        })
    }

    pub fn head(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, IoError> {
        log::debug!("performing libcurl HEAD to {url}");

        let h = self.0.lock().expect("aquire lock on curl easy handle");

        h.reset_request()?;

        let url_c = CString::new(url)?;
        let method = CString::new("HEAD").unwrap();
        h.set_ptr(CURLOPT_URL, url_c.as_ptr() as *const c_void)?;
        h.set_long(CURLOPT_NOBODY, 1)?;
        h.set_ptr(CURLOPT_CUSTOMREQUEST, method.as_ptr() as *const c_void)?;

        let slist = build_headers(headers)?;
        h.set_ptr(CURLOPT_HTTPHEADER, slist.as_ptr() as *const c_void)?;

        let mut headers = Vec::new();
        let mut body = Vec::new();
        h.perform(&mut headers, &mut body)?;

        Ok(Response {
            status: h.response_code(),
            headers,
            body,
        })
    }

    pub fn put(
        &self,
        url: &str,
        data: &[u8],
        headers: &[(&str, &str)],
    ) -> Result<Response, IoError> {
        log::debug!("performing libcurl PUT to {url}");

        let h = self.0.lock().expect("aquire lock on curl easy handle");
        h.reset_request()?;

        let url_c = CString::new(url)?;
        h.set_ptr(CURLOPT_URL, url_c.as_ptr() as *const c_void)?;
        h.set_long(CURLOPT_UPLOAD, 1)?;
        h.set_off_t(CURLOPT_INFILESIZE_LARGE, data.len() as curl_off_t)?;

        let mut state = callbacks::ReadState { data, offset: 0 };
        h.set_ptr(CURLOPT_READFUNCTION, callbacks::read_cb as *const c_void)?;
        h.set_ptr(
            CURLOPT_READDATA,
            &mut state as *mut callbacks::ReadState<'_> as *mut c_void,
        )?;

        let mut extra = build_headers(headers)?;
        extra.append(&format!("Content-Length: {}", data.len()))?;
        extra.append("Content-Type: application/octet-stream")?;
        h.set_ptr(CURLOPT_HTTPHEADER, extra.as_ptr() as *const c_void)?;

        let mut headers = Vec::new();
        let mut body = Vec::new();
        h.perform(&mut headers, &mut body)?;

        Ok(Response {
            status: h.response_code(),
            headers,
            body,
        })
    }
}

struct Slist(*mut curl_slist);

impl Slist {
    fn new() -> Self {
        Self(ptr::null_mut())
    }

    fn append(&mut self, s: &str) -> Result<(), IoError> {
        let cs = CString::new(s)?;
        let next = unsafe { curl_slist_append(self.0, cs.as_ptr()) };
        if next.is_null() {
            Err(IoError::other("libcurl curl_slist_append failed"))
        } else {
            self.0 = next;
            Ok(())
        }
    }

    fn as_ptr(&self) -> *mut curl_slist {
        self.0
    }
}

impl Drop for Slist {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { curl_slist_free_all(self.0) };
        }
    }
}

mod callbacks {
    use super::*;

    pub(super) extern "C" fn write_cb(
        ptr: *mut libc::c_char,
        size: libc::size_t,
        nmemb: libc::size_t,
        data: *mut c_void,
    ) -> libc::size_t {
        let buf = unsafe { std::slice::from_raw_parts(ptr as *const u8, size * nmemb) };
        let vec = unsafe { &mut *(data as *mut Vec<u8>) };
        vec.extend_from_slice(buf);
        size * nmemb
    }

    pub(super) extern "C" fn header_cb(
        ptr: *mut libc::c_char,
        size: libc::size_t,
        nmemb: libc::size_t,
        data: *mut c_void,
    ) -> libc::size_t {
        let total = size * nmemb;
        let buf = unsafe { std::slice::from_raw_parts(ptr as *const u8, total) };
        if let Ok(s) = std::str::from_utf8(buf) {
            let trimmed = s.trim_end();
            if !trimmed.is_empty() {
                let vec = unsafe { &mut *(data as *mut Vec<String>) };
                vec.push(trimmed.to_owned());
            }
        }
        total
    }

    pub(super) struct ReadState<'a> {
        pub(super) data: &'a [u8],
        pub(super) offset: usize,
    }

    pub(super) extern "C" fn read_cb(
        ptr: *mut libc::c_char,
        size: libc::size_t,
        nmemb: libc::size_t,
        data: *mut c_void,
    ) -> libc::size_t {
        let state = unsafe { &mut *(data as *mut ReadState<'_>) };
        let buf = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, size * nmemb) };
        let remaining = &state.data[state.offset..];
        let n = remaining.len().min(buf.len());
        buf[..n].copy_from_slice(&remaining[..n]);
        state.offset += n;
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn http_get_success() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("GET").path(format!("/test"));
            then.status(200).body(&[123]);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let response = client.get(&srv.url("/test"), &[]).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, &[123]);
    }

    #[test]
    fn http_head_success() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("HEAD").path(format!("/test"));
            then.status(200);
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let response = client.head(&srv.url("/test"), &[]).unwrap();
        assert_eq!(response.status, 200);
    }

    #[test]
    fn http_put_success() {
        let srv = MockServer::start();
        srv.mock(|when, then| {
            when.method("PUT").path(format!("/test"));
            then.respond_with(|req| {
                // httpmock struggles with returning a body so just check it here
                assert!(req.body().contains_slice(b"foobar"));
                HttpMockResponse::builder().status(204).build()
            });
        });

        let client = CurlHttpClient::new("0.0.0").unwrap();
        let response = client.put(&srv.url("/test"), b"foobar", &[]).unwrap();
        assert_eq!(response.status, 204);
    }

    #[test]
    fn shared_handle_does_not_leak_between_requests() {
        let srv = MockServer::start();

        for i in 0..2 {
            srv.mock(|when, then| {
                when.method("PUT")
                    .path(format!("/put{i}"))
                    .header("x-id", format!("t{i}"))
                    .body(format!("payload{i}"));
                then.status(204);
            });
            srv.mock(|when, then| {
                when.method("GET")
                    .path(format!("/get{i}"))
                    .header_missing("x-id");
                then.status(200).body(format!("get{i}"));
            });
        }

        let client = CurlHttpClient::new("0.0.0").unwrap();

        let threads: Vec<_> = (0..2)
            .map(|i| {
                let c = client.clone();
                let put_url = srv.url(format!("/put{i}"));
                let get_url = srv.url(format!("/get{i}"));
                std::thread::spawn(move || {
                    for _ in 0..10 {
                        let id = format!("t{i}");
                        let put = c
                            .put(&put_url, format!("payload{i}").as_bytes(), &[("x-id", &id)])
                            .unwrap();
                        assert_eq!(put.status, 204);

                        let get = c.get(&get_url, &[]).unwrap();
                        assert_eq!(get.status, 200);
                        assert_eq!(get.body, format!("get{i}").into_bytes());
                    }
                })
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }
    }
}
