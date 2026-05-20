use crate::{ctr_gfx::*, screens::ScreenId};

pub fn header(ctx: &DrawContext, cur_screen: ScreenId) {
    ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
    ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);

    ctx.icon(ICON_LIST, 178.0 - 64.0, 0.0, 1.0);
    ctx.icon(ICON_CLOUD, 178.0, 0.0, 1.0);
    ctx.icon(ICON_INFO, 178.0 + 64.0, 0.0, 1.0);

    match cur_screen {
        ScreenId::Titles => {
            ctx.rect(178.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
            ctx.rect(178.0 + 64.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
        }
        ScreenId::Sync => {
            ctx.rect(178.0 - 64.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
            ctx.rect(178.0 + 64.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
        }
        ScreenId::Help => {
            ctx.rect(178.0 - 64.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
            ctx.rect(178.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
        }
    }

    ctx.text(6.0, 0.0, 1.0, WHITE, "\u{E004}");
    ctx.text(TOP_W - 28.0, 0.0, 1.0, WHITE, "\u{E005}");
}
