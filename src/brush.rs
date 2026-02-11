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

/// Minimal 2D brush state (rectangle), in local pixel coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BrushRect {
    active: bool,
    start_x: f32,
    start_y: f32,
    cur_x: f32,
    cur_y: f32,
}

impl BrushRect {
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn anchor_px(&self) -> Option<(f32, f32)> {
        if self.active {
            Some((self.start_x, self.start_y))
        } else {
            None
        }
    }

    pub(crate) fn begin(&mut self, x_px: f32, y_px: f32) {
        self.active = true;
        self.start_x = x_px;
        self.start_y = y_px;
        self.cur_x = x_px;
        self.cur_y = y_px;
    }

    pub(crate) fn update(&mut self, x_px: f32, y_px: f32) {
        if self.active {
            self.cur_x = x_px;
            self.cur_y = y_px;
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.active = false;
    }

    pub(crate) fn rect_px(&self) -> Option<(f32, f32, f32, f32)> {
        if !self.active {
            return None;
        }
        let x0 = self.start_x.min(self.cur_x);
        let x1 = self.start_x.max(self.cur_x);
        let y0 = self.start_y.min(self.cur_y);
        let y1 = self.start_y.max(self.cur_y);
        Some((x0, y0, x1, y1))
    }

    pub(crate) fn take_final_px(&mut self) -> Option<(f32, f32, f32, f32)> {
        let r = self.rect_px();
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

    #[test]
    fn brush_rect_tracks_rect() {
        let mut b = BrushRect::default();
        assert_eq!(b.rect_px(), None);
        b.begin(10.0, 5.0);
        assert_eq!(b.rect_px(), Some((10.0, 5.0, 10.0, 5.0)));
        b.update(3.0, 8.0);
        assert_eq!(b.rect_px(), Some((3.0, 5.0, 10.0, 8.0)));
        assert_eq!(b.take_final_px(), Some((3.0, 5.0, 10.0, 8.0)));
        assert_eq!(b.rect_px(), None);
    }
}
