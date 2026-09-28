/// Adapted from https://github.com/LumaTeam/Luma3DS/blob/master/sysmodules/rosalina/source/csvc.s#L101
use ctru_sys::{Handle, R_FAILED, Result};
use std::{arch::asm, io, sync::OnceLock};

static SESSION: OnceLock<Handle> = OnceLock::new();

pub struct FsPxi;

impl FsPxi {
    pub fn new() -> io::Result<Self> {
        if SESSION.get().is_some() {
            log::info!("PxiFS0 session is already initialised");
            return Ok(Self);
        }

        let mut h: Handle = 0;
        let res: Result;

        unsafe {
            asm!(
                "svc 0xB0",
                inout("r0") 0 => res,
                inout("r1") &mut h as *mut Handle => _,
                inout("r2") c"PxiFS0".as_ptr() => _,
                out("r3") _,
                out("r12") _,
                options(nostack),
            )
        }

        if R_FAILED(res) || h == 0 {
            log::info!(
                "unable to steal PxiFS0 session (requires Luma3DS) [{:#010X}]",
                res
            );
        } else {
            log::info!("stole pxi session, gba support enabled");
            SESSION.set(h).ok();
        }

        Ok(Self)
    }

    pub fn handle() -> io::Result<Handle> {
        SESSION.get().copied().ok_or(io::const_error!(
            io::ErrorKind::Unsupported,
            "pxi session not available, gba support disabled"
        ))
    }
}
