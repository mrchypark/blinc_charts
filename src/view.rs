use crate::scale::LinearScale;
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

/// Precomputed affine mapping from data space into plot-local pixel space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotAffine {
    x_domain_origin: f64,
    x_px_origin: f64,
    x_scale: f64,
    y_domain_origin: f64,
    y_px_origin: f64,
    y_scale: f64,
}

impl PlotAffine {
    pub fn new(domain: Domain2D, plot_x: f32, plot_y: f32, plot_w: f32, plot_h: f32) -> Self {
        Self {
            x_domain_origin: domain.x.min as f64,
            x_px_origin: plot_x as f64,
            x_scale: axis_scale(domain.x.min, domain.x.max, plot_w),
            y_domain_origin: domain.y.min as f64,
            y_px_origin: (plot_y + plot_h) as f64,
            y_scale: axis_scale(domain.y.min, domain.y.max, -plot_h),
        }
    }

    pub fn map_x(&self, x: f32) -> f32 {
        ((x as f64 - self.x_domain_origin) * self.x_scale + self.x_px_origin) as f32
    }

    pub fn map_y(&self, y: f32) -> f32 {
        ((y as f64 - self.y_domain_origin) * self.y_scale + self.y_px_origin) as f32
    }

    pub fn map_point(&self, p: Point) -> Point {
        Point::new(self.map_x(p.x), self.map_y(p.y))
    }
}

fn axis_scale(domain_min: f32, domain_max: f32, range_span: f32) -> f64 {
    let domain_span = (domain_max - domain_min) as f64;
    if domain_span.abs() < 1e-12 {
        0.0
    } else {
        range_span as f64 / domain_span
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

    pub fn plot_affine(&self, plot_x: f32, plot_y: f32, plot_w: f32, plot_h: f32) -> PlotAffine {
        PlotAffine::new(self.domain, plot_x, plot_y, plot_w, plot_h)
    }

    pub fn x_to_px(&self, x: f32, plot_x: f32, plot_w: f32) -> f32 {
        LinearScale::new(
            self.domain.x.min,
            self.domain.x.max,
            plot_x,
            plot_x + plot_w,
        )
        .map(x)
    }

    pub fn y_to_px(&self, y: f32, plot_y: f32, plot_h: f32) -> f32 {
        // y increases downward in screen coords.
        LinearScale::new(
            self.domain.y.min,
            self.domain.y.max,
            plot_y + plot_h,
            plot_y,
        )
        .map(y)
    }

    pub fn px_to_x(&self, px: f32, plot_x: f32, plot_w: f32) -> f32 {
        let px = px.clamp(plot_x, plot_x + plot_w);
        LinearScale::new(
            self.domain.x.min,
            self.domain.x.max,
            plot_x,
            plot_x + plot_w,
        )
        .invert(px)
    }

    pub fn px_to_y(&self, py: f32, plot_y: f32, plot_h: f32) -> f32 {
        let py = py.clamp(plot_y, plot_y + plot_h);
        LinearScale::new(
            self.domain.y.min,
            self.domain.y.max,
            plot_y + plot_h,
            plot_y,
        )
        .invert(py)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1e-4,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn plot_affine_matches_existing_point_mapping() {
        let view = ChartView {
            domain: Domain2D::new(
                Domain1D::new(1_700_000_000.0, 1_700_086_400.0),
                Domain1D::new(-125.5, 987.25),
            ),
            padding: [0.0; 4],
        };
        let (plot_x, plot_y, plot_w, plot_h) = (12.0, 24.0, 640.0, 320.0);
        let affine = view.plot_affine(plot_x, plot_y, plot_w, plot_h);
        let points = [
            Point::new(view.domain.x.min, view.domain.y.min),
            Point::new(view.domain.x.max, view.domain.y.max),
            Point::new(1_700_021_600.0, -20.25),
            Point::new(1_700_043_200.0, 512.0),
            Point::new(1_700_064_800.0, 900.5),
        ];

        for point in points {
            let legacy = view.data_to_px(point, plot_x, plot_y, plot_w, plot_h);
            let mapped = affine.map_point(point);
            assert_close(mapped.x, legacy.x);
            assert_close(mapped.y, legacy.y);
        }
    }
}
