/// CURL wrapper around a patched curl-sys (libcurl).
///
/// It is possible to use libraries like ureq and minreq on this hardware target,
/// but it has many problems, namely inability to use HTTPS or timeouts.
/// CURL based options are available from devkitpro so we use those.
///
/// *I did not write this wrapper. It was almost totally written by an LLM and
/// essentially reimplements parts of the higher level curl crate, which fails
/// to compile on this target. Credit where credit's due, it handled this
/// very well when coralled toward the light.*
use curl_sys::*;
use libc::c_void;
use std::ffi::CString;
use std::ptr;

const CAINFO: &str = "romfs:/cacert.pem";

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Error {
    Curl(u32),
    NulByte,
    Other,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Curl(code) => write!(f, "libcurl error {code}"),
            Error::NulByte => write!(f, "nul byte in argument"),
            Error::Other => write!(f, "other error"),
        }
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(_: std::ffi::NulError) -> Self {
        Error::NulByte
    }
}

fn check(code: CURLcode) -> Result<(), Error> {
    if code == CURLE_OK {
        Ok(())
    } else {
        Err(Error::Curl(code as u32))
    }
}

// ── Response ─────────────────────────────────────────────────────────────────

pub struct Response {
    pub status: u32,
    pub headers: Vec<String>,
    pub body: Vec<u8>,
}

// ── SList ───────────────────────────────────────────────────────────────

struct Slist(*mut curl_slist);

impl Slist {
    fn new() -> Self {
        Self(ptr::null_mut())
    }

    fn append(&mut self, s: &str) -> Result<(), Error> {
        let cs = CString::new(s)?;
        let next = unsafe { curl_slist_append(self.0, cs.as_ptr()) };
        if next.is_null() {
            Err(Error::Curl(CURLE_OUT_OF_MEMORY as u32))
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

// ── Callbacks ────────────────────────────────────────────────────────────────

extern "C" fn write_cb(
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

extern "C" fn header_cb(
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

struct ReadState<'a> {
    data: &'a [u8],
    offset: usize,
}

extern "C" fn read_cb(
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

// ── Client ───────────────────────────────────────────────────────────────────

pub struct Client {
    handle: *mut CURL,
}

// Single-threaded CTR homebrew only.
unsafe impl Send for Client {}
unsafe impl Sync for Client {}

impl Client {
    pub fn new() -> Result<Self, Error> {
        let cainfo_c = CString::new(CAINFO)?;
        let handle = unsafe { curl_easy_init() };
        if handle.is_null() {
            return Err(Error::Other);
        }

        // Options that never change across requests.
        unsafe {
            curl_easy_setopt(handle, CURLOPT_CAINFO, cainfo_c.as_ptr() as *const c_void);
            curl_easy_setopt(handle, CURLOPT_TIMEOUT, 30_i64);
            curl_easy_setopt(handle, CURLOPT_CONNECTTIMEOUT, 10_i64);
            curl_easy_setopt(handle, CURLOPT_FOLLOWLOCATION, 1_i64);
            curl_easy_setopt(handle, CURLOPT_MAXREDIRS, 5_i64);
            curl_easy_setopt(handle, CURLOPT_WRITEFUNCTION, write_cb as *const c_void);
            curl_easy_setopt(handle, CURLOPT_HEADERFUNCTION, header_cb as *const c_void);
        }

        // cainfo_c must outlive the setopt call. Since CURLOPT_CAINFO copies
        // the string internally, dropping it here is safe.
        drop(cainfo_c);

        Ok(Self { handle })
    }

    fn set_long(&self, opt: CURLoption, val: libc::c_long) -> Result<(), Error> {
        check(unsafe { curl_easy_setopt(self.handle, opt, val) })
    }

    fn set_ptr(&self, opt: CURLoption, val: *const c_void) -> Result<(), Error> {
        check(unsafe { curl_easy_setopt(self.handle, opt, val) })
    }

    fn set_off_t(&self, opt: CURLoption, val: curl_off_t) -> Result<(), Error> {
        check(unsafe { curl_easy_setopt(self.handle, opt, val) })
    }

    fn response_code(&self) -> u32 {
        let mut code: libc::c_long = 0;
        unsafe { curl_easy_getinfo(self.handle, CURLINFO_RESPONSE_CODE, &mut code) };
        code as u32
    }

    // Reset per-request state, keeping persistent options (CAINFO, timeouts,
    // callbacks, connection pool) intact.
    fn reset_request(&self) -> Result<(), Error> {
        // curl_easy_reset clears everything including our persistent options,
        // so we instead just clear the per-request options explicitly.
        self.set_long(CURLOPT_HTTPGET, 0)?;
        self.set_long(CURLOPT_NOBODY, 0)?;
        self.set_long(CURLOPT_UPLOAD, 0)?;
        self.set_long(CURLOPT_POST, 0)?;
        self.set_ptr(CURLOPT_CUSTOMREQUEST, ptr::null())?;
        self.set_ptr(CURLOPT_READFUNCTION, ptr::null())?;
        self.set_ptr(CURLOPT_READDATA, ptr::null())?;
        self.set_off_t(CURLOPT_INFILESIZE_LARGE, -1)?;
        Ok(())
    }

    fn build_headers(&self, extra: &[(&str, &str)]) -> Result<Slist, Error> {
        let mut slist = Slist::new();
        // Drop Connection: close now that we want keepalive.
        for (k, v) in extra {
            slist.append(&format!("{k}: {v}"))?;
        }
        Ok(slist)
    }

    fn perform(&self, body: &mut Vec<u8>, headers: &mut Vec<String>) -> Result<(), Error> {
        self.set_ptr(CURLOPT_WRITEDATA, body as *mut Vec<u8> as *mut c_void)?;
        self.set_ptr(
            CURLOPT_HEADERDATA,
            headers as *mut Vec<String> as *mut c_void,
        )?;
        check(unsafe { curl_easy_perform(self.handle) })
    }

    pub fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, Error> {
        self.reset_request()?;

        let url_c = CString::new(url)?;
        self.set_ptr(CURLOPT_URL, url_c.as_ptr() as *const c_void)?;
        self.set_long(CURLOPT_HTTPGET, 1)?;

        let slist = self.build_headers(headers)?;
        self.set_ptr(CURLOPT_HTTPHEADER, slist.as_ptr() as *const c_void)?;

        let mut body = Vec::new();
        let mut hdrs = Vec::new();
        self.perform(&mut body, &mut hdrs)?;

        Ok(Response {
            status: self.response_code(),
            headers: hdrs,
            body,
        })
    }

    pub fn head(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, Error> {
        self.reset_request()?;

        let url_c = CString::new(url)?;
        let method = CString::new("HEAD").unwrap();
        self.set_ptr(CURLOPT_URL, url_c.as_ptr() as *const c_void)?;
        self.set_long(CURLOPT_NOBODY, 1)?;
        self.set_ptr(CURLOPT_CUSTOMREQUEST, method.as_ptr() as *const c_void)?;

        let slist = self.build_headers(headers)?;
        self.set_ptr(CURLOPT_HTTPHEADER, slist.as_ptr() as *const c_void)?;

        let mut body = Vec::new();
        let mut hdrs = Vec::new();
        self.perform(&mut body, &mut hdrs)?;

        Ok(Response {
            status: self.response_code(),
            headers: hdrs,
            body,
        })
    }

    pub fn put(&self, url: &str, data: &[u8], headers: &[(&str, &str)]) -> Result<Response, Error> {
        self.reset_request()?;

        let url_c = CString::new(url)?;
        self.set_ptr(CURLOPT_URL, url_c.as_ptr() as *const c_void)?;
        self.set_long(CURLOPT_UPLOAD, 1)?;
        self.set_off_t(CURLOPT_INFILESIZE_LARGE, data.len() as curl_off_t)?;

        let mut state = ReadState { data, offset: 0 };
        self.set_ptr(CURLOPT_READFUNCTION, read_cb as *const c_void)?;
        self.set_ptr(
            CURLOPT_READDATA,
            &mut state as *mut ReadState<'_> as *mut c_void,
        )?;

        let mut extra = self.build_headers(headers)?;
        extra.append(&format!("Content-Length: {}", data.len()))?;
        extra.append("Content-Type: application/octet-stream")?;
        self.set_ptr(CURLOPT_HTTPHEADER, extra.as_ptr() as *const c_void)?;

        let mut body = Vec::new();
        let mut hdrs = Vec::new();
        self.perform(&mut body, &mut hdrs)?;

        Ok(Response {
            status: self.response_code(),
            headers: hdrs,
            body,
        })
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe { curl_easy_cleanup(self.handle) };
    }
}
