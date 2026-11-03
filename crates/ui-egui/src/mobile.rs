//! Compact runtime projection. This never installs or saves a different desktop dock tree.

use egui::{Align, Align2, Rect, UiBuilder, pos2, vec2};
use serde_json::{Value, json};

use crate::FilmcraftApp as NativeApp;
use crate::mobile_policy::{LayoutChoice, Policy};
use crate::mobile_touch::TouchRouter;

pub const PREFERENCE_FILE: &str = "aca-mobile-layout-v1.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Preview,
    Timeline,
    Project,
    Properties,
    Tools,
}

impl Pane {
    pub fn name(self) -> &'static str {
        match self {
            Self::Preview => "Preview",
            Self::Timeline => "Timeline",
            Self::Project => "Project",
            Self::Properties => "Properties",
            Self::Tools => "Tools",
        }
    }
}

pub struct Runtime {
    pub policy: Policy,
    pub pane: Pane,
    pub navigate: bool,
    pub timeline_visible: bool,
    pub timeline: Option<crate::state::TimelineView>,
    pub viewport: Rect,
    pub bounds: Rect,
    pub touch: TouchRouter,
    pub buttons: Vec<(String, Rect)>,
    pub context: Option<(u64, Option<u64>)>,
    pub boundary_pending: bool,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            policy: Policy::default(),
            pane: Pane::Preview,
            navigate: false,
            timeline_visible: true,
            timeline: None,
            viewport: Rect::NOTHING,
            bounds: Rect::NOTHING,
            touch: TouchRouter::default(),
            buttons: Vec::new(),
            context: None,
            boundary_pending: false,
        }
    }
}

impl Runtime {
    pub fn owns_input(&self) -> bool {
        self.touch.active() || self.boundary_pending
    }

    pub fn load_preference(&mut self, text: &str) {
        if text.len() > 256 {
            return;
        }
        if let Ok(value) = serde_json::from_str::<Value>(text)
            && value.get("version").and_then(Value::as_u64) == Some(1)
            && let Some(choice) = value.get("layout").and_then(Value::as_str).and_then(LayoutChoice::parse)
        {
            self.policy.choice = choice;
        }
    }

    pub fn preference(&self) -> String {
        json!({"version":1,"layout":self.policy.choice.name()}).to_string()
    }

    pub fn describe(&self) -> Value {
        let rect = |r: Rect| if r.is_finite() { json!([r.min.x, r.min.y, r.width(), r.height()]) } else { Value::Null };
        json!({
            "version":1,
            "applied":if self.policy.compact {"Compact"} else {"Desktop"},
            "preferred":self.policy.choice.name(),
            "pane":self.pane.name(),
            "interaction":if self.navigate {"Navigate"} else {"Edit"},
            "viewport":rect(self.viewport),
            "workspace":rect(self.bounds),
            "touchBlocked":self.touch.ownership.blocked,
            "inputOwned":self.owns_input(),
            "boundaryPending":self.boundary_pending,
            "buttons":self.buttons.iter().map(|(id,r)| json!({"id":id,"rect":rect(*r)})).collect::<Vec<_>>()
        })
    }
}

impl NativeApp {
    pub(crate) fn mobile_update(&mut self, ui: &egui::Ui) {
        self.mobile_sync_context();
        let rect = ui.max_rect();
        self.mobile.viewport = rect;
        let gesture = ui.input(|i| i.pointer.any_down()) || self.mobile.owns_input();
        self.mobile.policy.update(rect.width(), rect.height(), gesture);
        if !self.mobile.policy.compact {
            self.mobile.buttons.clear();
        }
    }

    fn mobile_button(&mut self, ui: &mut egui::Ui, id: &str, label: &str, selected: bool) -> bool {
        let response = ui.add(egui::Button::new(label).selected(selected).min_size(vec2(44.0, 44.0)));
        self.auto.add(&format!("mobile.{id}"), response.rect, label);
        self.mobile.buttons.push((id.to_string(), response.rect));
        response.clicked()
    }

    fn mobile_layout_choice(&mut self, ui: &mut egui::Ui) {
        let before = self.mobile.policy.choice;
        let response = egui::ComboBox::from_id_salt("aca-mobile-layout").selected_text(self.mobile.policy.choice.name()).width(78.0).show_ui(ui, |ui| {
            ui.spacing_mut().interact_size = vec2(44.0, 44.0);
            for choice in [LayoutChoice::Auto, LayoutChoice::Compact, LayoutChoice::Desktop] {
                ui.selectable_value(&mut self.mobile.policy.choice, choice, choice.name());
            }
        });
        self.auto.add("mobile.layout", response.response.rect, "Workspace layout");
        self.mobile.buttons.push(("layout".into(), response.response.rect));
        if self.mobile.policy.choice != before {
            self.mobile_store_layout();
            ui.ctx().request_repaint();
        }
    }

    pub(crate) fn mobile_desktop_selector(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::unique("aca-layout-selector")).anchor(Align2::RIGHT_BOTTOM, vec2(-6.0, -6.0)).order(egui::Order::Foreground).show(
            ctx,
            |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.spacing_mut().interact_size = vec2(44.0, 44.0);
                    self.mobile_layout_choice(ui);
                });
            },
        );
    }

    pub(crate) fn mobile_draw(&mut self, ui: &mut egui::Ui, full: Rect) {
        self.mobile.buttons.clear();
        let header = Rect::from_min_size(full.min, vec2(full.width(), 88.0));
        let mut bar = ui.new_child(UiBuilder::new().max_rect(header).layout(egui::Layout::top_down(Align::Min)).id_salt("aca-mobile-toolbar"));
        bar.set_clip_rect(header.intersect(ui.clip_rect()));
        bar.spacing_mut().item_spacing = vec2(3.0, 0.0);
        bar.spacing_mut().interact_size = vec2(44.0, 44.0);
        bar.horizontal(|ui| {
            for (id, label, command) in
                [("open", "Open", "file.open"), ("save", "Save", "file.save"), ("undo", "Undo", "edit.undo"), ("redo", "Redo", "edit.redo")]
            {
                if self.mobile_button(ui, id, label, false) {
                    self.mobile_command(ui.ctx(), command);
                }
            }
            let menu = ui.menu_button("More", |ui| {
                ui.spacing_mut().interact_size = vec2(44.0, 44.0);
                for (label, command) in [("New", "@new"), ("Import media", "file.import"), ("Export frame", "@export")] {
                    if ui.add(egui::Button::new(label).min_size(vec2(160.0, 44.0))).clicked() {
                        self.mobile_command(ui.ctx(), command);
                        ui.close();
                    }
                }
                if ui.add(egui::Button::new("Show / hide timeline").min_size(vec2(160.0, 44.0))).clicked() {
                    self.mobile.timeline_visible = !self.mobile.timeline_visible;
                    ui.close();
                }
                ui.separator();
                egui::ScrollArea::horizontal().max_width(280.0).show(ui, |ui| crate::menus::menu_bar(self, ui));
            });
            self.auto.add("mobile.more", menu.response.rect, "More commands");
            self.mobile.buttons.push(("more".into(), menu.response.rect));
        });
        bar.horizontal(|ui| {
            if self.mobile_button(ui, "play", "Play", false) {
                self.mobile_command(ui.ctx(), "@play");
            }
            if self.mobile_button(ui, "stop", "Stop", false) {
                self.mobile_command(ui.ctx(), "@stop");
            }
            if self.mobile_button(ui, "edit", "Edit", !self.mobile.navigate) {
                self.mobile.navigate = false;
            }
            if self.mobile_button(ui, "navigate", "Navigate", self.mobile.navigate) {
                self.mobile.navigate = true;
            }
            self.mobile_layout_choice(ui);
        });
        let tab_top = header.max.y.min(full.max.y);
        let tabs = Rect::from_min_max(pos2(full.min.x, tab_top), pos2(full.max.x, (tab_top + 44.0).min(full.max.y)));
        let mut row = ui.new_child(UiBuilder::new().max_rect(tabs).layout(egui::Layout::left_to_right(Align::Center)).id_salt("aca-mobile-panes"));
        row.set_clip_rect(tabs.intersect(ui.clip_rect()));
        row.spacing_mut().item_spacing.x = 2.0;
        for (pane, label) in
            [(Pane::Preview, "Preview"), (Pane::Timeline, "Timeline"), (Pane::Project, "Project"), (Pane::Properties, "Props"), (Pane::Tools, "Tools")]
        {
            if self.mobile_button(&mut row, &format!("pane.{}", pane.name()), label, self.mobile.pane == pane) {
                self.mobile.pane = pane;
            }
        }
        let body = Rect::from_min_max(pos2(full.min.x, tabs.max.y), full.max);
        self.mobile.bounds = body;
        if body.width() < 1.0 || body.height() < 1.0 {
            return;
        }
        if self.mobile.pane == Pane::Preview && self.mobile.timeline_visible && body.height() >= 460.0 {
            let split = body.min.y + body.height() * 0.54;
            self.mobile_panel(ui, Pane::Preview, Rect::from_min_max(body.min, pos2(body.max.x, split - 2.0)));
            self.mobile_panel(ui, Pane::Timeline, Rect::from_min_max(pos2(body.min.x, split + 2.0), body.max));
        } else {
            self.mobile_panel(ui, self.mobile.pane, body);
        }
    }
}
