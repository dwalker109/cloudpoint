mod c2d;
mod draw;
mod icons;

use crate::screens::{BaseScreen, ModalScreen};
use c2d::*;
pub use draw::DrawContext;
pub use icons::*;

const GFX_TOP: gfxScreen_t = gfxScreen_t_GFX_TOP;
const GFX_BOTTOM: gfxScreen_t = gfxScreen_t_GFX_BOTTOM;
const GFX_LEFT: gfx3dSide_t = gfx3dSide_t_GFX_LEFT;

pub const TOP_W: f32 = 400.0;
pub const TOP_H: f32 = 240.0;
pub const BOT_W: f32 = 320.0;
pub const BOT_H: f32 = 240.0;

pub const WHITE: u32 = 0xFFFFFFFF;
pub const BLACK: u32 = 0xFF000000;
pub const GREY: u32 = 0xFFCCCCCC;
pub const GREY_TRANS: u32 = 0xAACCCCCC;
pub const DARK_GREY: u32 = 0xFF888888;
pub const ACCENT: u32 = 0xFFF986DB;
pub const ACCENT_TRANS: u32 = 0xAAF986DB;

pub struct Render {
    upper_screen: *mut C3D_RenderTarget,
    lower_screen: *mut C3D_RenderTarget,
    text_buf: C2D_TextBuf,
}

impl Render {
    pub fn new() -> Self {
        log::debug!("initialising renderer");

        unsafe {
            C3D_Init(C3D_DEFAULT_CMDBUF_SIZE as usize);
            C2D_Init(C2D_DEFAULT_MAX_OBJECTS as usize);
            C2D_Prepare();
            Self {
                upper_screen: C2D_CreateScreenTarget(GFX_TOP, GFX_LEFT),
                lower_screen: C2D_CreateScreenTarget(GFX_BOTTOM, GFX_LEFT),
                text_buf: C2D_TextBufNew(1024),
            }
        }
    }

    pub fn frame(&mut self, screen: &dyn BaseScreen, modal: Option<&dyn ModalScreen>) {
        let ctx = DrawContext::new(self.text_buf);
        unsafe {
            C3D_FrameBegin(C3D_FRAME_SYNCDRAW as u8);
            C2D_TargetClear(self.upper_screen, WHITE);
            C2D_SceneBegin(self.upper_screen);
            screen.draw_upper(&ctx);

            if let Some(m) = modal {
                ctx.rect(0.0, 0.0, TOP_W, TOP_H, GREY_TRANS);
                m.draw_upper(&ctx);
            }

            C2D_TargetClear(self.lower_screen, WHITE);
            C2D_SceneBegin(self.lower_screen);
            screen.draw_lower(&ctx);

            if let Some(m) = modal {
                ctx.rect(0.0, 0.0, BOT_W, BOT_H, GREY_TRANS);
                m.draw_lower(&ctx);
            }

            C3D_FrameEnd(0);
        }
    }
}

impl Drop for Render {
    fn drop(&mut self) {
        log::debug!("dropping renderer");

        unsafe {
            C2D_TextBufDelete(self.text_buf);
            C2D_Fini();
            C3D_Fini();
        }
    }
}
