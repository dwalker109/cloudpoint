use super::c2d::*;
use std::ffi::CString;

struct SpriteSheet(C2D_SpriteSheet);

impl SpriteSheet {
    fn load(path: &str) -> Option<Self> {
        let cs = CString::new(path).ok()?;
        let sheet = unsafe { C2D_SpriteSheetLoad(cs.as_ptr()) };
        if sheet.is_null() {
            None
        } else {
            Some(Self(sheet))
        }
    }

    fn image(&self, index: usize) -> C2D_Image {
        unsafe { C2D_SpriteSheetGetImage(self.0, index) }
    }
}

impl Drop for SpriteSheet {
    fn drop(&mut self) {
        unsafe {
            C2D_SpriteSheetFree(self.0);
        }
    }
}

pub struct DrawContext {
    buf: C2D_TextBuf,
    glyph_w: f32,
    icons: SpriteSheet,
}

impl DrawContext {
    pub(crate) fn new(buf: C2D_TextBuf) -> Self {
        let icons = SpriteSheet::load("romfs:/icons.t3x").expect("should load icons spritesheet");

        let glyph_w = unsafe {
            let measure_buf = C2D_TextBufNew(16);
            let cs = CString::new("\u{E000}").unwrap();
            let mut t: C2D_Text = std::mem::zeroed();
            C2D_TextParse(&mut t, measure_buf, cs.as_ptr());
            C2D_TextOptimize(&t);
            let mut gw: f32 = 0.0;
            let mut gh: f32 = 0.0;
            C2D_TextGetDimensions(&t, 1.0, 1.0, &mut gw, &mut gh);
            C2D_TextBufDelete(measure_buf);
            gw
        };

        Self {
            buf,
            glyph_w,
            icons,
        }
    }

    pub fn icon(&self, icon_index: u32, x: f32, y: f32, scale: f32) {
        let img = self.icons.image(icon_index as usize);
        unsafe {
            C2D_DrawImageAt(img, x, y, 0.5, std::ptr::null(), scale, scale);
        }
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
            let padding = if contains_glyph(s) {
                self.glyph_w * scale * 0.5
            } else {
                0.0
            };
            let tx = x + (w - (tw + padding)) / 2.0;
            let ty = y;
            C2D_DrawText(
                &t,
                C2D_WithColor as u32,
                tx,
                ty,
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
        let (_, th) = self.text_dimensions(scale, label);
        let ty = y + (h - th) / 2.0;
        self.text_centered(x, ty, w, scale, fg, label);
    }

    pub fn text_dimensions(&self, scale: f32, s: &str) -> (f32, f32) {
        unsafe {
            let cs = CString::new(s).unwrap_or_default();
            let mut t: C2D_Text = std::mem::zeroed();
            C2D_TextBufClear(self.buf);
            C2D_TextParse(&mut t, self.buf, cs.as_ptr());
            C2D_TextOptimize(&t);
            let mut tw: f32 = 0.0;
            let mut th: f32 = 0.0;
            C2D_TextGetDimensions(&t, scale, scale, &mut tw, &mut th);
            (tw, th)
        }
    }
}

fn contains_glyph(s: &str) -> bool {
    s.chars().any(|c| c >= '\u{E000}' && c <= '\u{E0FF}')
}
