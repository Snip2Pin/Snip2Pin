//! Menu ownership is independent of which native window receives the input.
use eframe::egui;

#[derive(Clone, Copy, Default)]
pub struct MenuInput {
    open: bool,
    consumed: Option<(egui::ViewportId, u64)>,
}

impl MenuInput {
    fn key() -> egui::Id {
        egui::Id::unique("pin-menu-input")
    }

    /// Publish changes immediately so previously created deferred callbacks see
    /// them too. Dismissal owns its input viewport's frame, including UI re-passes.
    /// A hidden root need not paint before a pin can receive fresh input again.
    pub fn sync(ctx: &egui::Context, open: bool) {
        let stamp = (ctx.viewport_id(), ctx.cumulative_frame_nr());
        ctx.data_mut(|data| {
            let mut state = data.get_temp::<Self>(Self::key()).unwrap_or_default();
            if state.open && !open && state.consumed.is_none() {
                state.consumed = Some(stamp);
            } else if !state.open && open {
                state.consumed = None;
            }
            state.open = open;
            data.insert_temp(Self::key(), state);
        });
    }

    /// Retain the actual input owner before a deferred root action closes the menu.
    pub fn consume(ctx: &egui::Context) {
        let stamp = (ctx.viewport_id(), ctx.cumulative_frame_nr());
        ctx.data_mut(|data| {
            if let Some(mut state) = data.get_temp::<Self>(Self::key()) {
                state.consumed = Some(stamp);
                data.insert_temp(Self::key(), state);
            }
        });
    }

    pub fn open(ctx: &egui::Context) -> bool {
        ctx.data(|data| data.get_temp::<Self>(Self::key()))
            .is_some_and(|state| state.open)
    }

    pub fn blocks_input(ctx: &egui::Context) -> bool {
        let stamp = (ctx.viewport_id(), ctx.cumulative_frame_nr());
        ctx.data(|data| data.get_temp::<Self>(Self::key()))
            .is_some_and(|state| state.open || state.consumed == Some(stamp))
    }
}
