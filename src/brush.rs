/// Minimal 1D brush state (X axis), in local pixel coordinates.
///
/// We keep this in pixel space for two reasons:
/// - gesture handling is naturally in pixels
/// - domain conversion depends on the current view transform
#[derive(Clone, Copy, Debug, Default)]
pub struct BrushX {
    active: bool,
    start_px: f32,
    cur_px: f32,
}

impl BrushX {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn anchor_px(&self) -> Option<f32> {
        if self.active {
            Some(self.start_px)
        } else {
            None
        }
    }

    pub fn begin(&mut self, x_px: f32) {
        self.active = true;
        self.start_px = x_px;
        self.cur_px = x_px;
    }

    pub fn update(&mut self, x_px: f32) {
        if self.active {
            self.cur_px = x_px;
        }
    }

    pub fn cancel(&mut self) {
        self.active = false;
    }

    pub fn range_px(&self) -> Option<(f32, f32)> {
        if !self.active {
            return None;
        }
        let (a, b) = (self.start_px, self.cur_px);
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    pub fn take_final_px(&mut self) -> Option<(f32, f32)> {
        let r = self.range_px();
        self.active = false;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brush_tracks_range() {
        let mut b = BrushX::default();
        assert_eq!(b.range_px(), None);
        b.begin(10.0);
        assert_eq!(b.range_px(), Some((10.0, 10.0)));
        b.update(3.0);
        assert_eq!(b.range_px(), Some((3.0, 10.0)));
        assert_eq!(b.take_final_px(), Some((3.0, 10.0)));
        assert_eq!(b.range_px(), None);
    }
}
