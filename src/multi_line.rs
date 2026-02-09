use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, CornerRadius, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::lod::{downsample_min_max, DownsampleParams};
use crate::segments::runs_by_gap;
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};

/// Visual styling for a multi-line chart.
#[derive(Clone, Debug)]
pub struct MultiLineChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub crosshair: Color,
    pub text: Color,

    pub stroke_width: f32,
    pub series_alpha: f32,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,

    /// Maximum number of series to draw as lines.
    ///
    /// (If you want a 10k-series overview, you'll likely want a density renderer instead.)
    pub max_series: usize,

    /// Hard budget for the total number of line segments we emit per frame.
    ///
    /// This avoids overflowing the GPU line segment buffer (default ~50k).
    pub max_total_segments: usize,

    /// Cap for per-series downsample output. Actual per-series points may be lower due to
    /// `max_total_segments` budgeting.
    pub max_points_per_series: usize,
}

impl Default for MultiLineChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            stroke_width: 1.0,
            series_alpha: 0.18,
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
            max_series: 1_000,
            max_total_segments: 45_000,
            max_points_per_series: 2_048,
        }
    }
}

/// Mutable model for an interactive multi-line chart.
///
/// This intentionally uses a single scratch buffer reused across series to keep memory use low.
pub struct MultiLineChartModel {
    pub series: Vec<TimeSeriesF32>,
    pub view: ChartView,
    pub style: MultiLineChartStyle,

    /// If finite, consecutive points with `dx > gap_dx` will not be connected.
    /// Use this to "break" lines when samples are missing.
    pub gap_dx: f32,

    pub crosshair_x: Option<f32>, // local px in plot area

    // EventRouter drag deltas are "offset from drag start".
    last_drag_total_x: Option<f32>,

    scratch_data: Vec<Point>, // data coords
    scratch_px: Vec<Point>,   // local px coords
    scratch_runs: Vec<(usize, usize)>,
    downsample_params: DownsampleParams,
}

impl MultiLineChartModel {
    pub fn new(series: Vec<TimeSeriesF32>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !series.is_empty(),
            "MultiLineChartModel requires at least 1 series"
        );

        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut y_min = f32::INFINITY;
        let mut y_max = f32::NEG_INFINITY;
        for s in &series {
            let (sx0, sx1) = s.x_min_max();
            x_min = x_min.min(sx0);
            x_max = x_max.max(sx1);
            let (sy0, sy1) = s.y_min_max();
            y_min = y_min.min(sy0);
            y_max = y_max.max(sy1);
        }

        // Avoid degenerate y ranges.
        if !(y_max > y_min) {
            // Handle degenerate or invalid y-ranges.
            if y_min.is_finite() && y_max.is_finite() {
                y_min -= 1.0;
                y_max += 1.0;
            } else {
                // Fallback for non-finite ranges (e.g. all NaN data).
                y_min = -1.0;
                y_max = 1.0;
            }
        }

        let domain = Domain2D::new(Domain1D::new(x_min, x_max), Domain1D::new(y_min, y_max));
        Ok(Self {
            series,
            view: ChartView::new(domain),
            style: MultiLineChartStyle::default(),
            gap_dx: f32::INFINITY,
            crosshair_x: None,
            last_drag_total_x: None,
            scratch_data: Vec::new(),
            scratch_px: Vec::new(),
            scratch_runs: Vec::new(),
            downsample_params: DownsampleParams::default(),
        })
    }

    pub fn set_gap_dx(&mut self, gap_dx: f32) {
        self.gap_dx = gap_dx;
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.crosshair_x = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.crosshair_x = None;
            return;
        }
        self.crosshair_x = Some(local_x);
    }

    pub fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) {
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);

        let delta_y = delta_y.clamp(-250.0, 250.0);
        let zoom = (-delta_y * self.style.scroll_zoom_factor).exp();
        self.view.domain.x.zoom_about(pivot_x, zoom);
        self.view.domain.x.clamp_span_min(1e-6);
    }

    pub fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) {
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);

        let zoom = scale_delta.max(self.style.pinch_zoom_min);
        self.view.domain.x.zoom_about(pivot_x, zoom);
        self.view.domain.x.clamp_span_min(1e-6);
    }

    /// Pan using drag "total delta from start" (EventContext::drag_delta_x).
    pub fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        let (_px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return;
        }
        // Convert total-from-start to incremental delta since last event.
        let prev = self.last_drag_total_x.replace(drag_total_dx);
        let drag_dx = match prev {
            Some(p) => drag_total_dx - p,
            None => 0.0,
        };

        // Convert pixel delta to domain delta.
        let dx = -drag_dx / pw * self.view.domain.x.span();
        self.view.domain.x.pan_by(dx);
    }

    pub fn on_drag_end(&mut self) {
        self.last_drag_total_x = None;
    }

    fn palette_color(i: usize, alpha: f32) -> Color {
        // Golden-ratio hue step for decent distribution.
        let h = (i as f32 * 0.618_033_988_75) % 1.0;
        let s = 0.75;
        let v = 0.95;
        let (r, g, b) = hsv_to_rgb(h, s, v);
        Color::rgba(r, g, b, alpha)
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        ctx.fill_rect(
            Rect::new(0.0, 0.0, w, h),
            CornerRadius::default(),
            Brush::Solid(self.style.bg),
        );

        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        // Grid (cheap, fixed count)
        let grid_n = 4;
        for i in 0..=grid_n {
            let t = i as f32 / grid_n as f32;
            let x = px + t * pw;
            let y = py + t * ph;
            ctx.fill_rect(
                Rect::new(x, py, 1.0, ph),
                0.0.into(),
                Brush::Solid(self.style.grid),
            );
            ctx.fill_rect(
                Rect::new(px, y, pw, 1.0),
                0.0.into(),
                Brush::Solid(self.style.grid),
            );
        }

        let n = self.series.len().min(self.style.max_series);
        if n == 0 {
            return;
        }

        let stroke = Stroke::new(self.style.stroke_width);
        let mut remaining_segments = self.style.max_total_segments.max(1);

        // Per-series point cap: also bounded by pixels so we don't waste work.
        let px_cap = (pw.ceil() as usize).saturating_mul(2).clamp(64, 200_000);
        let hard_per_series_cap = self.style.max_points_per_series.min(px_cap);

        for (si, s) in self.series.iter().take(n).enumerate() {
            if remaining_segments == 0 {
                break;
            }

            // Budget segments fairly across remaining series.
            let remaining_series = (n - si).max(1);
            let seg_budget = (remaining_segments / remaining_series).max(8);
            let point_budget = (seg_budget + 1).clamp(2, hard_per_series_cap);

            self.downsample_params.max_points = point_budget;
            downsample_min_max(
                s,
                self.view.domain.x.min,
                self.view.domain.x.max,
                self.downsample_params,
                &mut self.scratch_data,
            );

            if self.scratch_data.len() < 2 {
                continue;
            }

            // Convert to px.
            self.scratch_px.clear();
            self.scratch_px.reserve(self.scratch_data.len());
            for p in &self.scratch_data {
                self.scratch_px
                    .push(self.view.data_to_px(*p, px, py, pw, ph));
            }

            // Split runs on missing data gaps.
            runs_by_gap(&self.scratch_data, self.gap_dx, &mut self.scratch_runs);

            let color = Self::palette_color(si, self.style.series_alpha);
            for (a, b) in self.scratch_runs.iter().copied() {
                let len = b.saturating_sub(a);
                if len < 2 || remaining_segments == 0 {
                    continue;
                }

                // Clip to segment budget.
                let need = len - 1;
                if need > remaining_segments {
                    let end = a + remaining_segments + 1;
                    if end > a + 1 && end <= b {
                        ctx.stroke_polyline(&self.scratch_px[a..end], &stroke, Brush::Solid(color));
                        remaining_segments = 0;
                    }
                    break;
                } else {
                    ctx.stroke_polyline(&self.scratch_px[a..b], &stroke, Brush::Solid(color));
                    remaining_segments = remaining_segments.saturating_sub(need);
                }
            }
        }
    }

    pub fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        if let Some(cx) = self.crosshair_x {
            let x = cx.clamp(px, px + pw);
            ctx.fill_rect(
                Rect::new(x, py, 1.0, ph),
                0.0.into(),
                Brush::Solid(self.style.crosshair),
            );

            let xv = self.view.px_to_x(x, px, pw);
            let text = format!("x={:.3}", xv);
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_empty_series() {
        assert!(MultiLineChartModel::new(Vec::new()).is_err());
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = (h.fract() + 1.0).fract() * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// Shared handle for a multi-line chart model.
#[derive(Clone)]
pub struct MultiLineChartHandle(pub Arc<Mutex<MultiLineChartModel>>);

impl MultiLineChartHandle {
    pub fn new(model: MultiLineChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

/// Create an interactive multi-line chart element.
///
/// Interactions:
/// - Scroll/pinch: zoom X about cursor
/// - Drag: pan X
pub fn multi_line_chart(handle: MultiLineChartHandle) -> impl ElementBuilder {
    let model_plot = handle.0.clone();
    let model_overlay = handle.0.clone();

    let model_move = handle.0.clone();
    let model_scroll = handle.0.clone();
    let model_pinch = handle.0.clone();
    let model_drag = handle.0.clone();
    let model_drag_end = handle.0.clone();

    stack()
        .w_full()
        .h_full()
        .overflow_clip()
        .cursor(blinc_layout::element::CursorStyle::Crosshair)
        .on_mouse_move(move |e| {
            if let Ok(mut m) = model_move.lock() {
                m.on_mouse_move(e.local_x, e.local_y, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_scroll(move |e| {
            if let Ok(mut m) = model_scroll.lock() {
                m.on_scroll(e.scroll_delta_y, e.local_x, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_pinch(move |e| {
            if let Ok(mut m) = model_pinch.lock() {
                m.on_pinch(e.pinch_scale, e.local_x, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_drag(move |e| {
            if let Ok(mut m) = model_drag.lock() {
                m.on_drag_pan_total(e.drag_delta_x, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_drag_end(move |_e| {
            if let Ok(mut m) = model_drag_end.lock() {
                m.on_drag_end();
            }
        })
        .child(
            canvas(move |ctx, bounds| {
                if let Ok(mut m) = model_plot.lock() {
                    m.render_plot(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full(),
        )
        .child(
            canvas(move |ctx, bounds| {
                if let Ok(mut m) = model_overlay.lock() {
                    m.render_overlay(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full()
            .foreground(),
        )
}
