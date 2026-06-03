use crate::{ctr_gfx::*, screens::ScreenId};
use std::{sync::LazyLock, time::Instant};

pub fn header(ctx: &DrawContext, cur_screen: ScreenId) {
    ctx.rect(0.0, 0.0, TOP_W, TOP_H, WHITE);
    ctx.rect(0.0, 0.0, TOP_W, 32.0, ACCENT);

    ctx.icon(ICON_LIST, 178.0 - 64.0, 0.0, 1.0);
    ctx.icon(ICON_CLOUD, 178.0, 0.0, 1.0);
    ctx.icon(ICON_LINK, 178.0 + 64.0, 0.0, 1.0);

    match cur_screen {
        ScreenId::Titles => {
            ctx.rect(178.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
            ctx.rect(178.0 + 64.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
        }
        ScreenId::Sync => {
            ctx.rect(178.0 - 64.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
            ctx.rect(178.0 + 64.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
        }
        ScreenId::Link => {
            ctx.rect(178.0 - 64.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
            ctx.rect(178.0, 0.0, 32.0, 32.0, ACCENT_TRANS);
        }
    }

    ctx.text(6.0, 0.0, 1.0, WHITE, "\u{E004}");
    ctx.text(TOP_W - 28.0, 0.0, 1.0, WHITE, "\u{E005}");
}

static SPINSTANT: LazyLock<Instant> = LazyLock::new(|| Instant::now());

static SPINNER_A: [&'static str; 8] = [
    "\u{E020}", "\u{E021}", "\u{E022}", "\u{E023}", "\u{E024}", "\u{E025}", "\u{E026}", "\u{E027}",
];
static SPINNER_B: [&'static str; 8] = [
    "\u{E023}", "\u{E024}", "\u{E025}", "\u{E026}", "\u{E027}", "\u{E020}", "\u{E021}", "\u{E022}",
];

pub fn modal_spinner(ctx: &DrawContext) {
    ctx.text(
        TOP_W - 60.0,
        30.0,
        1.2,
        ACCENT,
        SPINNER_A[(SPINSTANT.elapsed().as_millis() / 150) as usize % SPINNER_A.len()],
    );
    ctx.text(
        TOP_W - 60.0,
        30.0,
        1.2,
        ACCENT_TRANS,
        SPINNER_B[(SPINSTANT.elapsed().as_millis() / 150) as usize % SPINNER_B.len()],
    );
}
