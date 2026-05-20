use itertools::Itertools;

use super::*;

pub struct HelpScreen;

impl HelpScreen {
    pub fn new() -> Self {
        Self
    }
}

impl Screen for HelpScreen {
    fn draw_upper(&self, ctx: &DrawContext) {
        super::shared::header(ctx, self.id());

        ctx.text_centered(0.0, 40.0, TOP_W, 1.0, ACCENT, "\u{E010} Important \u{E010}");
        ctx.text(
            30.0,
            78.0,
            0.5,
            BLACK,
            &[
                "Cloudpoint reads, writes, uploads and downloads your",
                "saves and extdata. No warranty is implied or offered.",
                "While great care has been taken to avoid data loss,",
                "you must ensure you back up your saves yourself.\n",
                "Take regular backups with a local tool, like Checkpoint,",
                "and keep them somewhere safe. I use Cloudpoint daily",
                "and have no issues, but that isn't a guarantee nothing",
                "can go wrong. You should not rely on it as your sole",
                "source of long term storage; the data could disappear!",
            ]
            .iter()
            .join("\n"),
        );
    }

    fn draw_lower(&self, ctx: &DrawContext) {
        ctx.rect(0.0, 0.0, BOT_W, BOT_H, ACCENT);
        ctx.text_centered(
            0.0,
            14.0,
            BOT_W,
            0.48,
            WHITE,
            &[
                "Visit our GitHub for setup guides, help, and FAQs.",
                "Please also join our Discord! Let's be friends.",
            ]
            .iter()
            .join("\n"),
        );

        ctx.icon(ICON_HELP_QR, 70.0, 60.0, 0.75);
    }
}

impl BaseScreen for HelpScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Help
    }

    fn handle_msg(&mut self, _msg: &UiMsg) -> ScreenCommand {
        ScreenCommand::Noop
    }

    fn handle_input(&mut self, keys_down: &KeyPad, _keys_held: &KeyPad) -> ScreenCommand {
        if keys_down.contains(KeyPad::L) {
            return ScreenCommand::SwitchTo(ScreenId::Sync);
        }

        ScreenCommand::Noop
    }
}
