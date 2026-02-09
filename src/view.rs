use blinc_core::Point;

/// 1D numeric domain (min..max).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Domain1D {
    pub min: f32,
    pub max: f32,
}

impl Domain1D {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    pub fn span(&self) -> f32 {
        self.max - self.min
    }

    pub fn is_valid(&self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.max > self.min
    }

    pub fn clamp_span_min(&mut self, min_span: f32) {
        let span = self.span();
        if span < min_span {
            let mid = (self.min + self.max) * 0.5;
            self.min = mid - min_span * 0.5;
            self.max = mid + min_span * 0.5;
        }
    }

    pub fn pan_by(&mut self, delta: f32) {
        self.min += delta;
        self.max += delta;
    }

    /// Zoom in/out about a pivot position (domain units).
    ///
    /// `factor > 1` zooms in, `factor < 1` zooms out.
    pub fn zoom_about(&mut self, pivot: f32, factor: f32) {
        let factor = factor.max(0.0001);
        let min = pivot + (self.min - pivot) / factor;
        let max = pivot + (self.max - pivot) / factor;
        self.min = min;
        self.max = max;
    }
}

/// 2D domain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Domain2D {
    pub x: Domain1D,
    pub y: Domain1D,
}

impl Domain2D {
    pub fn new(x: Domain1D, y: Domain1D) -> Self {
        Self { x, y }
    }

    pub fn is_valid(&self) -> bool {
        self.x.is_valid() && self.y.is_valid()
    }
}

/// View transform for a chart: data domain mapping to local pixel space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartView {
    pub domain: Domain2D,
    /// Padding inside the chart plotting area (left, top, right, bottom).
    pub padding: [f32; 4],
}

impl ChartView {
    pub fn new(domain: Domain2D) -> Self {
        Self {
            domain,
            padding: [32.0, 16.0, 16.0, 24.0],
        }
    }

    pub fn plot_rect(&self, width: f32, height: f32) -> (f32, f32, f32, f32) {
        let left = self.padding[0];
        let top = self.padding[1];
        let right = self.padding[2];
        let bottom = self.padding[3];
        let w = (width - left - right).max(0.0);
        let h = (height - top - bottom).max(0.0);
        (left, top, w, h)
    }

    pub fn x_to_px(&self, x: f32, plot_x: f32, plot_w: f32) -> f32 {
        let t = (x - self.domain.x.min) / self.domain.x.span();
        plot_x + t * plot_w
    }

    pub fn y_to_px(&self, y: f32, plot_y: f32, plot_h: f32) -> f32 {
        // y increases downward in screen coords.
        let t = (y - self.domain.y.min) / self.domain.y.span();
        plot_y + (1.0 - t) * plot_h
    }

    pub fn px_to_x(&self, px: f32, plot_x: f32, plot_w: f32) -> f32 {
        let t = ((px - plot_x) / plot_w).clamp(0.0, 1.0);
        self.domain.x.min + t * self.domain.x.span()
    }

    pub fn px_to_y(&self, py: f32, plot_y: f32, plot_h: f32) -> f32 {
        let t = ((py - plot_y) / plot_h).clamp(0.0, 1.0);
        self.domain.y.min + (1.0 - t) * self.domain.y.span()
    }

    pub fn data_to_px(
        &self,
        p: Point,
        plot_x: f32,
        plot_y: f32,
        plot_w: f32,
        plot_h: f32,
    ) -> Point {
        Point::new(
            self.x_to_px(p.x, plot_x, plot_w),
            self.y_to_px(p.y, plot_y, plot_h),
        )
    }
}
