/// Interaction bindings for charts.
///
/// `blinc_charts` is a library crate, so gesture/key bindings must be configurable.
/// We keep bindings simple and purely data-driven so they can be shared across chart types.

/// A "required modifiers" mask.
///
/// If a field is `true`, the corresponding key must be held. Non-required keys are ignored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModifiersReq {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl ModifiersReq {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Self::default()
        }
    }

    pub fn matches(&self, shift: bool, ctrl: bool, alt: bool, meta: bool) -> bool {
        (!self.shift || shift)
            && (!self.ctrl || ctrl)
            && (!self.alt || alt)
            && (!self.meta || meta)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragAction {
    None,
    PanX,
    BrushX,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DragBinding {
    pub required: ModifiersReq,
    pub action: DragAction,
}

impl DragBinding {
    pub fn none() -> Self {
        Self {
            required: ModifiersReq::none(),
            action: DragAction::None,
        }
    }
}

/// Common interaction bindings shared by charts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChartInputBindings {
    /// Which drag gesture is interpreted as a brush.
    pub brush_drag: DragBinding,
    /// Which drag gesture is interpreted as pan (when not brushing).
    pub pan_drag: DragBinding,
}

impl Default for ChartInputBindings {
    fn default() -> Self {
        Self {
            brush_drag: DragBinding {
                required: ModifiersReq::shift(),
                action: DragAction::BrushX,
            },
            pan_drag: DragBinding {
                required: ModifiersReq::none(),
                action: DragAction::PanX,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_req_matches_required_only() {
        let r = ModifiersReq::shift();
        assert!(r.matches(true, false, false, false));
        // Extra modifiers should not prevent a match.
        assert!(r.matches(true, true, false, true));
        assert!(!r.matches(false, true, false, false));
    }
}

