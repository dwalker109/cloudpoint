use anyhow::Result;

pub fn get_friend_code_seed() -> Result<u64> {
    let seed = ffi::cfgi_get_local_friend_code_seed()?;

    Ok(seed)
}

pub fn format_friend_code_seed(fc: u64) -> String {
    let a = fc / 100_000_000;
    let b = (fc / 10_000) % 10_000;
    let c = fc % 10_000;

    format!("{:04}-{:04}-{:04}", a, b, c)
}

mod ffi {
    use anyhow::{Result, bail};
    use ctru_sys::{R_FAILED, cfguExit, cfguInit};

    pub(super) fn cfgi_get_local_friend_code_seed() -> Result<u64> {
        let res = unsafe { cfguInit() };

        if R_FAILED(res) {
            bail!("could not initialise cfgu");
        }

        let mut seed: u64 = 0;

        let res = unsafe { ctru_sys::CFGI_GetLocalFriendCodeSeed(&mut seed) };

        if R_FAILED(res) {
            bail!("could not get local friend code seed");
        }

        unsafe { cfguExit() };

        Ok(seed)
    }
}
