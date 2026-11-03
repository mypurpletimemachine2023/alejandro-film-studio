//! Video Lab's compact adapter. Desktop docking and document view settings remain untouched.

use crate::dock::PanelKind;
use crate::mobile::Pane;
use crate::state::Tool;
use crate::{FilmcraftApp, menus, panels};
use egui::{Rect, UiBuilder, vec2};
use serde_json::json;

impl FilmcraftApp {
    pub(crate) fn mobile_sync_context(&mut self) {
        let context = (self.session.document_epoch, self.session.state.active_sequence.map(|id| id.0));
        if self.mobile.context != Some(context) {
            self.mobile.context = Some(context);
            self.mobile.timeline = None;
            self.mobile_checkpoint = None;
        }
    }

    pub(crate) fn mobile_store_layout(&mut self) {
        // Browser persistence is owned by the web host, separately from recovery and projects.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = self.session.prefs_path.as_ref().and_then(|path| path.parent()).map(|path| path.join(crate::mobile::PREFERENCE_FILE))
            && let Err(error) = std::fs::write(path, self.mobile.preference())
        {
            self.ui.status = format!("Layout changed for this session; preference could not be saved: {error}");
        }
    }

    pub(crate) fn mobile_raw_input(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if !self.mobile.policy.compact {
            return;
        }
        let route = self.mobile.touch.filter(raw, self.mobile.bounds);
        if route.started && !self.mobile.navigate && self.dialog.is_none() {
            self.mobile_checkpoint = Some(filmcraft_engine::gesture::Checkpoint::capture(&self.session));
        }
        if route.cancel {
            self.tl.drag = None;
            self.tl.pressed = None;
            self.tl.cancelled = true;
            self.value_press = None;
            panels::transform_handles::cancel_touch(ctx);
            panels::graphics::cancel_touch(ctx);
            if let Some(checkpoint) = self.mobile_checkpoint.take() {
                checkpoint.cancel(&mut self.session);
            }
        }
        if route.ended {
            self.mobile_checkpoint = None;
        }
    }

    pub(crate) fn mobile_command(&mut self, ctx: &egui::Context, command: &str) {
        if command == "@play" {
            self.toggle_play(1.0);
            return;
        }
        if command == "@stop" {
            self.stop();
            return;
        }
        let command = match command {
            "@new" => "file.newProject",
            "@export" => "file.exportFrame",
            command => command,
        };
        if let Err(error) = menus::invoke(self, ctx, command, json!({})) {
            self.ui.status = error;
        }
    }

    pub(crate) fn mobile_panel(&mut self, ui: &mut egui::Ui, pane: Pane, rect: Rect) {
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect).id_salt(("mobile-panel", pane.name())));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        child.spacing_mut().interact_size.y = 44.0;
        if self.mobile.touch.ownership.blocked {
            child.disable();
        }
        if pane == Pane::Tools {
            egui::ScrollArea::vertical().show(&mut child, |ui| {
                ui.heading("Video tools");
                for tool in Tool::ALL {
                    let response = ui.add(egui::Button::new(tool.label()).selected(self.ui.tool == tool).min_size(vec2(240.0, 44.0)));
                    self.auto.add(&format!("mobile.tool.{}", tool.label()), response.rect, tool.label());
                    if response.clicked() {
                        self.ui.tool = tool;
                        self.mobile.navigate = false;
                        self.mobile.pane = if tool == Tool::Razor { Pane::Timeline } else { Pane::Preview };
                    }
                }
            });
            return;
        }
        let panel = match pane {
            Pane::Preview => PanelKind::Program,
            Pane::Timeline => PanelKind::Timeline,
            Pane::Project => PanelKind::Project,
            Pane::Properties => PanelKind::Properties,
            Pane::Tools => return,
        };
        self.auto.add(&format!("panel.{}", panel.id()), rect, panel.title());
        let old_focus = self.ui.focused;
        let old_tool = self.ui.tool;
        self.ui.focused = panel;
        if self.mobile.navigate && matches!(pane, Pane::Preview | Pane::Timeline) {
            self.ui.tool = Tool::Hand;
        }
        if pane == Pane::Timeline {
            let mut compact = self.mobile.timeline.take().unwrap_or_else(|| self.ui.timeline.clone());
            compact.header_w = 120.0;
            compact.video_track_h = compact.video_track_h.max(44.0);
            compact.audio_track_h = compact.audio_track_h.max(44.0);
            let desktop = std::mem::replace(&mut self.ui.timeline, compact);
            panels::show(self, &mut child, panel, rect);
            self.mobile.timeline = Some(std::mem::replace(&mut self.ui.timeline, desktop));
        } else {
            panels::show(self, &mut child, panel, rect);
        }
        self.ui.focused = old_focus;
        self.ui.tool = old_tool;
    }
}
