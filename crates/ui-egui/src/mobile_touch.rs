//! Touch contact ownership shared by compact viewers. No document mutation happens here.

use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchId, TouchPhase, Vec2};

use crate::mobile_policy::TouchOwnership;

#[derive(Default)]
pub struct TouchRouter {
    contacts: Vec<(TouchId, Pos2)>,
    pub ownership: TouchOwnership,
    owned: bool,
    previous: Option<(Pos2, f32)>,
    native_cancel: Option<Event>,
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
                            let mut cancel = event.clone();
                            if let Event::Touch { phase, .. } = &mut cancel {
                                *phase = TouchPhase::Cancel;
                            }
                            self.native_cancel = Some(cancel);
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
                if let Some(cancel) = self.native_cancel.take() {
                    raw.events.push(cancel);
                }
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
        out.ended = (was_active || out.started) && !self.active();
        if !self.active() {
            self.owned = false;
            self.native_cancel = None;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(id: u64, phase: TouchPhase, x: f32) -> Event {
        Event::Touch { device_id: egui::TouchDeviceId(1), id: TouchId(id), phase, pos: Pos2::new(x, 100.0), force: None }
    }

    fn input(events: Vec<Event>) -> RawInput {
        RawInput { focused: true, events, ..RawInput::default() }
    }

    fn bounds() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 600.0))
    }

    #[test]
    fn academy_mobile_second_finger_cancels_the_native_contact_once() {
        let mut router = TouchRouter::default();
        assert!(router.filter(&mut input(vec![touch(1, TouchPhase::Start, 80.0)]), bounds()).started);
        let mut pinch = input(vec![touch(2, TouchPhase::Start, 200.0)]);
        assert!(router.filter(&mut pinch, bounds()).cancel);
        assert!(pinch.events.iter().any(|event| matches!(event, Event::Touch { id: TouchId(1), phase: TouchPhase::Cancel, .. })));
        assert!(!pinch.events.iter().any(|event| matches!(event, Event::Touch { phase: TouchPhase::Start, .. })));
        assert!(!router.filter(&mut input(vec![touch(2, TouchPhase::End, 200.0)]), bounds()).cancel);
        assert!(router.ownership.blocked);
        assert!(router.filter(&mut input(vec![touch(1, TouchPhase::End, 80.0)]), bounds()).ended);
        assert!(!router.ownership.blocked);
    }

    #[test]
    fn academy_mobile_batched_pinch_cannot_commit_a_click() {
        let mut router = TouchRouter::default();
        let mut raw = input(vec![
            touch(1, TouchPhase::Start, 80.0),
            touch(2, TouchPhase::Start, 200.0),
            touch(2, TouchPhase::End, 200.0),
            touch(1, TouchPhase::End, 80.0),
        ]);
        let result = router.filter(&mut raw, bounds());
        assert!(result.cancel && result.ended);
        assert!(!router.active());
        assert!(!router.ownership.blocked);
        assert!(!raw.events.iter().any(|event| matches!(event, Event::PointerButton { pressed: true, .. })));
    }

    #[test]
    fn academy_mobile_focus_loss_releases_and_recovers_input() {
        let mut router = TouchRouter::default();
        router.filter(&mut input(vec![touch(1, TouchPhase::Start, 80.0)]), bounds());
        let mut lost = RawInput { focused: false, ..RawInput::default() };
        assert!(router.filter(&mut lost, bounds()).cancel);
        assert!(!router.active());
        assert!(lost.events.iter().any(|event| matches!(event, Event::PointerGone)));
        router.filter(&mut input(vec![]), bounds());
        assert!(!router.ownership.blocked);
    }

    #[test]
    fn academy_mobile_tap_finishes_without_retaining_a_checkpoint() {
        let mut router = TouchRouter::default();
        let mut raw = input(vec![touch(1, TouchPhase::Start, 80.0), touch(1, TouchPhase::End, 80.0)]);
        let result = router.filter(&mut raw, bounds());
        assert!(result.started && result.ended && !result.cancel);
    }
}
