use super::c2d::*;
use std::ffi::CString;

pub struct DrawContext {
    buf: C2D_TextBuf,
}

impl DrawContext {
    pub(crate) fn new(buf: C2D_TextBuf) -> Self {
        Self { buf }
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, colour: u32) {
        unsafe {
            C2D_DrawRectSolid(x, y, 0.5, w, h, colour);
        }
    }

    pub fn text(&self, x: f32, y: f32, scale: f32, colour: u32, s: &str) {
        unsafe {
            let cs = CString::new(s).unwrap_or_default();
            let mut t: C2D_Text = std::mem::zeroed();
            C2D_TextBufClear(self.buf);
            C2D_TextParse(&mut t, self.buf, cs.as_ptr());
            C2D_TextOptimize(&t);
            C2D_DrawText(&t, C2D_WithColor as u32, x, y, 0.5, scale, scale, colour, 0);
        }
    }

    pub fn text_centered(&self, x: f32, y: f32, w: f32, scale: f32, colour: u32, s: &str) {
        unsafe {
            let cs = CString::new(s).unwrap_or_default();
            let mut t: C2D_Text = std::mem::zeroed();
            let mut tw: f32 = 0.0;
            let mut th: f32 = 0.0;
            C2D_TextBufClear(self.buf);
            C2D_TextParse(&mut t, self.buf, cs.as_ptr());
            C2D_TextOptimize(&t);
            C2D_TextGetDimensions(&t, scale, scale, &mut tw, &mut th);
            let tx = x + (w - tw) / 2.0;
            C2D_DrawText(
                &t,
                C2D_WithColor as u32,
                tx,
                y,
                0.5,
                scale,
                scale,
                colour,
                0,
            );
        }
    }

    pub fn button(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        bg: u32,
        fg: u32,
        label: &str,
        scale: f32,
    ) {
        self.rect(x, y, w, h, bg);
        let text_h = 14.0 * scale;
        let ty = y + (h - text_h) / 2.0;
        self.text_centered(x, ty, w, scale, fg, label);
    }
}
