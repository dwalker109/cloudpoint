use crate::screens::{BaseScreen, ModalScreen};
use c2d::*;

mod c2d;
mod draw;
mod img;

pub use draw::DrawContext;
pub use img::*;

const GFX_TOP: gfxScreen_t = gfxScreen_t_GFX_TOP;
const GFX_BOTTOM: gfxScreen_t = gfxScreen_t_GFX_BOTTOM;
const GFX_LEFT: gfx3dSide_t = gfx3dSide_t_GFX_LEFT;

pub const TOP_W: f32 = 400.0;
pub const TOP_H: f32 = 240.0;
pub const BOT_W: f32 = 320.0;
pub const BOT_H: f32 = 240.0;

pub const ROUND_RAD_SM: f32 = 3.0;

pub const WHITE: u32 = 0xFFFFFFFF;
pub const BLACK: u32 = 0xFF000000;
pub const BLACK_WASH: u32 = 0x33000000;
pub const GREY: u32 = 0xFFCCCCCC;
pub const DARK_GREY: u32 = 0xFF888888;
pub const ACCENT: u32 = 0xFFF986DB;
pub const ACCENT_TRANS: u32 = 0xAAF986DB;

pub struct Render {
    upper_screen: *mut C3D_RenderTarget,
    lower_screen: *mut C3D_RenderTarget,
    draw_context: DrawContext,
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
                draw_context: DrawContext::new(C2D_TextBufNew(1024)),
            }
        }
    }

    pub fn frame(&mut self, screen: &dyn BaseScreen, modal: Option<&dyn ModalScreen>) {
        unsafe {
            C3D_FrameBegin(C3D_FRAME_SYNCDRAW as u8);
            C2D_TargetClear(self.upper_screen, WHITE);
            C2D_SceneBegin(self.upper_screen);
            screen.draw_upper(&self.draw_context);

            if let Some(m) = modal {
                m.draw_upper(&self.draw_context);
            }

            C2D_TargetClear(self.lower_screen, WHITE);
            C2D_SceneBegin(self.lower_screen);
            screen.draw_lower(&self.draw_context);

            if let Some(m) = modal {
                m.draw_lower(&self.draw_context);
            }

            C3D_FrameEnd(0);
        }
    }
}

impl Drop for Render {
    fn drop(&mut self) {
        log::debug!("dropping renderer");

        unsafe {
            C2D_Fini();
            C3D_Fini();
        }
    }
}
