//! Narrow Academy appearance queue: native appearance preferences, never artwork commands.
use std::cell::Cell;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
thread_local! {
    #[cfg(target_arch = "wasm32")]
    static CONTEXT: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
    static PENDING: Cell<Option<bool>> = const { Cell::new(None) };
    static READY: Cell<bool> = const { Cell::new(false) };
    static CURRENT: Cell<bool> = const { Cell::new(false) };
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = academyTheme))]
pub fn academy_theme(light: bool) {
    PENDING.with(|p| p.set(Some(light)));
    #[cfg(target_arch = "wasm32")]
    CONTEXT.with(|slot| {
        if let Some(ctx) = slot.borrow().as_ref() {
            ctx.request_repaint();
        }
    });
}
/// Keep the currently owned graphics context so host changes also wake an idle editor.
#[cfg(target_arch = "wasm32")]
pub(crate) fn attach(ctx: &egui::Context) {
    CONTEXT.with(|slot| *slot.borrow_mut() = Some(ctx.clone()));
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = academyReady))]
pub fn academy_ready() -> bool {
    READY.with(Cell::get)
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = academyThemeState))]
pub fn academy_theme_state() -> bool {
    CURRENT.with(Cell::get)
}
pub(crate) fn take() -> Option<bool> {
    READY.with(|r| r.set(true));
    PENDING.with(Cell::take)
}
/// Acknowledge actual native tokens after preference and app logic have completed.
pub(crate) fn applied(light: bool) {
    CURRENT.with(|current| current.set(light));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latest_palette_request_wins_and_draining_does_not_replay_it() {
        academy_theme(true);
        academy_theme(false);
        academy_theme(true);
        assert_eq!(take(), Some(true));
        assert_eq!(take(), None);
        applied(true);
        assert!(academy_theme_state());
        assert!(academy_ready());
    }
    #[test]
    fn ready_is_explicit_and_both_palette_directions_are_reversible() {
        assert!(!academy_ready());
        assert_eq!(take(), None);
        assert!(academy_ready());
        academy_theme(true);
        assert_eq!(take(), Some(true));
        applied(true);
        assert!(academy_theme_state());
        academy_theme(false);
        assert_eq!(take(), Some(false));
        applied(false);
        assert!(!academy_theme_state());
    }
    #[test]
    fn requested_or_drained_intent_is_not_an_applied_palette() {
        assert!(!academy_theme_state());
        academy_theme(true);
        assert!(!academy_theme_state());
        assert_eq!(take(), Some(true));
        assert!(!academy_theme_state());
        applied(false);
        assert!(!academy_theme_state());
        applied(true);
        assert!(academy_theme_state());
        academy_theme(false);
        assert_eq!(take(), Some(false));
        assert!(academy_theme_state());
        applied(false);
        assert!(!academy_theme_state());
    }
}
