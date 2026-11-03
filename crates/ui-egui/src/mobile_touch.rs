//! Touch contact ownership shared by compact viewers. No document mutation happens here.

use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchId, TouchPhase, Vec2};

use crate::mobile_policy::TouchOwnership;

#[derive(Default)]
pub struct TouchRouter {
    contacts: Vec<(TouchId, Pos2)>,
    pub ownership: TouchOwnership,
    owned: bool,
    previous: Option<(Pos2, f32)>,
}

#[derive(Default)]
pub struct Route {
    pub started: bool,
    pub ended: bool,
    pub cancel: bool,
}

impl TouchRouter {
    pub fn active(&self) -> bool {
        !self.contacts.is_empty()
    }

    /// Retain touch ownership until every finger lifts. A released pointer is delivered only
    /// after the caller has cancelled its edit checkpoint and cleared in-progress handles.
    pub fn filter(&mut self, raw: &mut RawInput, bounds: Rect) -> Route {
        let was_active = self.active();
        let was_blocked = self.ownership.blocked;
        let mut out = Route::default();
        let mut cancelled = !raw.focused && was_active;
        let mut exceeded = false;
        let mut peak_contacts = self.contacts.len();
        for event in &raw.events {
            if let Event::Touch { id, phase, pos, .. } = event {
                if !pos.x.is_finite() || !pos.y.is_finite() {
                    cancelled = true;
                    self.contacts.clear();
                    continue;
                }
                match phase {
                    TouchPhase::Start => {
                        if self.contacts.is_empty() {
                            self.owned = bounds.contains(*pos);
                            out.started = self.owned;
                        }
                        if !self.contacts.iter().any(|(known, _)| known == id) {
                            if self.contacts.len() < 8 {
                                self.contacts.push((*id, *pos));
                            } else {
                                exceeded = true;
                            }
                        }
                    }
                    TouchPhase::Move => {
                        if let Some((_, previous)) = self.contacts.iter_mut().find(|(known, _)| known == id) {
                            *previous = *pos;
                        }
                    }
                    TouchPhase::End | TouchPhase::Cancel => {
                        cancelled |= *phase == TouchPhase::Cancel && self.owned;
                        self.contacts.retain(|(known, _)| known != id);
                    }
                }
                peak_contacts = peak_contacts.max(self.contacts.len());
            }
        }
        if !raw.focused {
            self.contacts.clear();
        }
        cancelled |= exceeded;
        let cancel_edit = self.ownership.update(peak_contacts, cancelled);
        out.cancel = self.owned && cancel_edit;
        if self.contacts.is_empty() && !cancelled {
            self.ownership.update(0, false);
        }
        let blocked = self.owned && (was_blocked || self.ownership.blocked || out.cancel);
        if blocked {
            raw.events.retain(|event| !matches!(event, Event::PointerButton { .. } | Event::PointerMoved(_) | Event::Touch { .. }));
            if out.cancel {
                let pos = self.contacts.first().map_or(bounds.center(), |(_, pos)| *pos);
                raw.events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
                raw.events.push(Event::PointerGone);
                raw.events.push(Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
                raw.events.push(Event::Key { key: egui::Key::Escape, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
            }
            if let (Some((_, a)), Some((_, b))) = (self.contacts.first(), self.contacts.get(1)) {
                let center = Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                let distance = a.distance(*b).max(1.0);
                raw.events.push(Event::PointerMoved(center));
                if let Some((previous, span)) = self.previous {
                    let delta = center - previous;
                    if delta.is_finite() && delta != Vec2::ZERO {
                        raw.events.push(Event::MouseWheel {
                            unit: MouseWheelUnit::Point,
                            delta,
                            modifiers: Modifiers::NONE,
                            phase: TouchPhase::Move,
                            source: MouseWheelSource::Trackpad,
                        });
                    }
                    let zoom = (distance / span).clamp(0.5, 2.0);
                    if zoom.is_finite() && (zoom - 1.0).abs() > 0.0001 {
                        raw.events.push(Event::Zoom(zoom));
                    }
                }
                self.previous = Some((center, distance));
            } else {
                self.previous = None;
            }
        } else {
            self.previous = None;
        }
        out.ended = was_active && !self.active();
        if !self.active() {
            self.owned = false;
        }
        out
    }
}
