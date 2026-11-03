//! Runtime presentation policy. It has no document, dock or renderer dependency.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutChoice {
    #[default]
    Auto,
    Compact,
    Desktop,
}

impl LayoutChoice {
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Compact => "Compact",
            Self::Desktop => "Desktop",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Auto" => Some(Self::Auto),
            "Compact" => Some(Self::Compact),
            "Desktop" => Some(Self::Desktop),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Policy {
    pub choice: LayoutChoice,
    pub compact: bool,
}

impl Policy {
    /// Hysteresis prevents virtual-keyboard and orientation jitter from repeatedly rebuilding
    /// the workspace. Even an explicit preference waits for the active gesture to finish.
    pub fn update(&mut self, width: f32, height: f32, gesture_active: bool) -> bool {
        if gesture_active || !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return false;
        }
        let want = match self.choice {
            LayoutChoice::Compact => true,
            LayoutChoice::Desktop => false,
            LayoutChoice::Auto if self.compact => !(width >= 960.0 && height >= 640.0),
            LayoutChoice::Auto => width < 900.0 || height < 600.0,
        };
        let changed = want != self.compact;
        self.compact = want;
        changed
    }
}

/// Once a second finger or cancellation takes ownership, the remaining finger must lift
/// before editing can resume. A pinch must never end in a stray primary-button edit.
#[derive(Clone, Debug, Default)]
pub struct TouchOwnership {
    pub blocked: bool,
}

impl TouchOwnership {
    pub fn update(&mut self, contacts: usize, cancelled: bool) -> bool {
        let cancel_edit = !self.blocked && (contacts >= 2 || cancelled);
        if contacts >= 2 || cancelled {
            self.blocked = true;
        }
        if contacts == 0 && !cancelled {
            self.blocked = false;
        }
        cancel_edit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_tablet_and_short_landscape_enter_compact() {
        for (w, h) in [(390.0, 744.0), (820.0, 1100.0), (844.0, 310.0), (1440.0, 599.0)] {
            let mut p = Policy::default();
            assert!(p.update(w, h, false));
            assert!(p.compact);
        }
    }

    #[test]
    fn exact_thresholds_have_hysteresis() {
        let mut p = Policy::default();
        assert!(!p.update(900.0, 600.0, false));
        assert!(p.update(899.0, 800.0, false));
        assert!(!p.update(959.0, 900.0, false));
        assert!(!p.update(1200.0, 639.0, false));
        assert!(p.compact);
        assert!(p.update(960.0, 640.0, false));
        assert!(!p.compact);
    }

    #[test]
    fn resize_and_override_wait_for_gesture_release() {
        let mut p = Policy::default();
        assert!(!p.update(390.0, 744.0, true));
        assert!(!p.compact);
        p.update(390.0, 744.0, false);
        p.choice = LayoutChoice::Desktop;
        assert!(!p.update(390.0, 744.0, true));
        assert!(p.compact);
        assert!(p.update(390.0, 744.0, false));
        assert!(!p.compact);
    }

    #[test]
    fn hostile_dimensions_preserve_last_valid_layout() {
        let mut p = Policy::default();
        p.update(390.0, 744.0, false);
        for (w, h) in [(f32::NAN, 800.0), (f32::INFINITY, 800.0), (0.0, 800.0), (1440.0, -1.0)] {
            assert!(!p.update(w, h, false));
            assert!(p.compact);
        }
    }

    #[test]
    fn preferences_are_allowlisted() {
        for choice in [LayoutChoice::Auto, LayoutChoice::Compact, LayoutChoice::Desktop] {
            assert_eq!(LayoutChoice::parse(choice.name()), Some(choice));
        }
        assert_eq!(LayoutChoice::parse("<script>"), None);
        assert_eq!(LayoutChoice::parse(""), None);
    }

    #[test]
    fn pinch_cancels_once_and_last_finger_cannot_edit() {
        let mut t = TouchOwnership::default();
        assert!(!t.update(1, false));
        assert!(t.update(2, false));
        assert!(!t.update(2, false));
        assert!(!t.update(1, false));
        assert!(t.blocked);
        assert!(!t.update(0, false));
        assert!(!t.blocked);
        assert!(!t.update(1, false));
    }

    #[test]
    fn cancellation_blocks_until_all_contacts_lift() {
        let mut t = TouchOwnership::default();
        assert!(t.update(0, true));
        assert!(t.blocked);
        t.update(1, false);
        assert!(t.blocked);
        t.update(0, false);
        assert!(!t.blocked);
    }
}
