use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::common::{draw_grid, fill_bg};
use crate::link::ChartLinkHandle;
use crate::palette;
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};
use crate::xy_stack::InteractiveXChartModel;

fn series_color(i: usize) -> Color {
    palette::qualitative(i, 0.85)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackedAreaMode {
    #[default]
    Stacked,
    Streamgraph,
}

#[derive(Clone, Debug)]
pub struct StackedAreaChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,
    pub crosshair: Color,

    pub mode: StackedAreaMode,
    pub stroke_width: f32,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,
}

impl Default for StackedAreaChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            mode: StackedAreaMode::Stacked,
            stroke_width: 1.0,
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CacheKey {
    x_min: u32,
    x_max: u32,
    y_min: u32,
    y_max: u32,
    plot_x: u32,
    plot_y: u32,
    plot_w: u32,
    plot_h: u32,
    mode: u8,
    series_n: u32,
}

impl CacheKey {
    fn new(model: &StackedAreaChartModel, plot: (f32, f32, f32, f32), series_n: usize) -> Self {
        let (px, py, pw, ph) = plot;
        let mode = match model.style.mode {
            StackedAreaMode::Stacked => 0,
            StackedAreaMode::Streamgraph => 1,
        };
        Self {
            x_min: model.view.domain.x.min.to_bits(),
            x_max: model.view.domain.x.max.to_bits(),
            y_min: model.view.domain.y.min.to_bits(),
            y_max: model.view.domain.y.max.to_bits(),
            plot_x: px.to_bits(),
            plot_y: py.to_bits(),
            plot_w: pw.to_bits(),
            plot_h: ph.to_bits(),
            mode,
            series_n: series_n as u32,
        }
    }
}

pub struct StackedAreaChartModel {
    pub series: Vec<TimeSeriesF32>,
    pub view: ChartView,
    pub style: StackedAreaChartStyle,

    pub crosshair_x: Option<f32>,
    pub hover_x: Option<f32>,

    last_drag_total_x: Option<f32>,
    brush_x: BrushX,

    // Cached geometry (local px) for plot rendering.
    cached_key: Option<CacheKey>,
    cached_sample_xs: Vec<f32>,
    cached_bottoms: Vec<f32>, // [s*sample_n + k]
    cached_tops: Vec<f32>,    // [s*sample_n + k]
    cached_band_paths: Vec<blinc_core::Path>,
    cached_top_pts: Vec<Vec<Point>>, // per-band polyline (px)
    scratch_vals: Vec<f32>,
}

impl StackedAreaChartModel {
    pub fn new(series: Vec<TimeSeriesF32>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !series.is_empty(),
            "StackedAreaChartModel requires at least 1 series"
        );
        anyhow::ensure!(
            series.iter().all(|s| !s.is_empty()),
            "StackedAreaChartModel requires non-empty series"
        );

        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        for s in &series {
            let (sx0, sx1) = s.x_min_max();
            if sx0.is_finite() {
                x_min = x_min.min(sx0);
            }
            if sx1.is_finite() {
                x_max = x_max.max(sx1);
            }
        }
        if !x_min.is_finite()
            || !x_max.is_finite()
            || x_max.partial_cmp(&x_min) != Some(std::cmp::Ordering::Greater)
        {
            x_min = 0.0;
            x_max = 1.0;
        }
        let domain = Domain2D::new(Domain1D::new(x_min, x_max), Domain1D::new(-1.0, 1.0));
        Ok(Self {
            series,
            view: ChartView::new(domain),
            style: StackedAreaChartStyle::default(),
            crosshair_x: None,
            hover_x: None,
            last_drag_total_x: None,
            brush_x: BrushX::default(),
            cached_key: None,
            cached_sample_xs: Vec::new(),
            cached_bottoms: Vec::new(),
            cached_tops: Vec::new(),
            cached_band_paths: Vec::new(),
            cached_top_pts: Vec::new(),
            scratch_vals: Vec::new(),
        })
    }

    fn y_at(series: &TimeSeriesF32, x: f32) -> f32 {
        if series.is_empty() {
            return 0.0;
        }
        let x_first = series.x[0];
        let x_last = series.x[series.len() - 1];
        if x < x_first || x > x_last {
            return 0.0;
        }

        let i = series.lower_bound_x(x);
        if i < series.len() && series.x[i] == x {
            let v = series.y[i];
            return if v.is_finite() { v } else { 0.0 };
        }
        if i == 0 || i >= series.len() {
            return 0.0;
        }

        let x0 = series.x[i - 1];
        let x1 = series.x[i];
        let y0 = series.y[i - 1];
        let y1 = series.y[i];
        if !x0.is_finite() || !x1.is_finite() || !y0.is_finite() || !y1.is_finite() || x1 <= x0 {
            return 0.0;
        }
        let t = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        y0 + (y1 - y0) * t
    }

    fn merged_x_samples(
        series: &[TimeSeriesF32],
        x_min: f32,
        x_max: f32,
        max_samples: Option<usize>,
    ) -> Vec<f32> {
        let mut xs = Vec::new();
        for s in series {
            if s.is_empty() {
                continue;
            }
            let mut i0 = s.lower_bound_x(x_min).min(s.len());
            if i0 > 0 {
                i0 -= 1;
            }
            let mut i1 = s.upper_bound_x(x_max).min(s.len());
            if i1 < s.len() {
                i1 += 1;
            }
            if i1 <= i0 {
                continue;
            }
            xs.extend(s.x[i0..i1].iter().copied().filter(|x| x.is_finite()));
        }
        if xs.is_empty() {
            return xs;
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        xs.dedup_by(|a, b| *a == *b);

        let Some(max_n) = max_samples else {
            return xs;
        };
        if max_n < 2 || xs.len() <= max_n {
            return xs;
        }

        let last = xs.len() - 1;
        let step = last as f32 / (max_n - 1) as f32;
        let mut out = Vec::with_capacity(max_n);
        for i in 0..max_n {
            let idx = ((i as f32 * step).round() as usize).min(last);
            out.push(xs[idx]);
        }
        out.dedup_by(|a, b| *a == *b);
        if out.len() < 2 {
            out.push(xs[last]);
        }
        out
    }

    fn compute_mode_bounds(
        series: &[TimeSeriesF32],
        mode: StackedAreaMode,
        x_min: f32,
        x_max: f32,
    ) -> (f32, f32) {
        let xs = Self::merged_x_samples(series, x_min, x_max, Some(2_048));
        if xs.is_empty() {
            return (-1.0, 1.0);
        }

        let mut out_min = f32::INFINITY;
        let mut out_max = f32::NEG_INFINITY;
        let mut vals = vec![0.0f32; series.len()];
        for x in xs {
            let mut pos_sum = 0.0f32;
            let mut neg_sum = 0.0f32;
            for (i, s) in series.iter().enumerate() {
                let v = Self::y_at(s, x);
                vals[i] = v;
                if v >= 0.0 {
                    pos_sum += v;
                } else {
                    neg_sum += v;
                }
            }

            match mode {
                StackedAreaMode::Stacked => {
                    out_min = out_min.min(neg_sum.min(0.0));
                    out_max = out_max.max(pos_sum.max(0.0));
                }
                StackedAreaMode::Streamgraph => {
                    let baseline = -0.5 * (pos_sum + neg_sum);
                    let mut cur = baseline;
                    out_min = out_min.min(cur);
                    out_max = out_max.max(cur);
                    for &v in &vals {
                        cur += v;
                        out_min = out_min.min(cur);
                        out_max = out_max.max(cur);
                    }
                }
            }
        }

        if !out_min.is_finite() || !out_max.is_finite() || out_max <= out_min {
            return (-1.0, 1.0);
        }
        if out_min > 0.0 {
            out_min = 0.0;
        }
        if out_max < 0.0 {
            out_max = 0.0;
        }
        if out_max <= out_min {
            out_max = out_min + 1.0;
        }
        (out_min, out_max)
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.crosshair_x = None;
            self.hover_x = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.crosshair_x = None;
            self.hover_x = None;
            return;
        }
        self.crosshair_x = Some(local_x);
        self.hover_x = Some(self.view.px_to_x(local_x, px, pw));
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

    pub fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        let (_px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return;
        }
        let prev = self.last_drag_total_x.replace(drag_total_dx);
        let drag_dx = match prev {
            Some(p) => drag_total_dx - p,
            None => 0.0,
        };
        let dx = -drag_dx / pw * self.view.domain.x.span();
        self.view.domain.x.pan_by(dx);
    }

    pub fn on_drag_end(&mut self) {
        self.last_drag_total_x = None;
    }

    pub fn on_mouse_down(&mut self, shift: bool, local_x: f32, w: f32, h: f32) {
        if !shift {
            return;
        }
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return;
        }
        self.brush_x.begin(local_x.clamp(px, px + pw));
        self.last_drag_total_x = None;
    }

    pub fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        if !self.brush_x.is_active() {
            return;
        }
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return;
        }
        if let Some(anchor) = self.brush_x.anchor_px() {
            self.brush_x
                .update((anchor + drag_total_dx).clamp(px, px + pw));
        }
    }

    pub fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            self.brush_x.cancel();
            return None;
        }
        let (a_px, b_px) = self.brush_x.take_final_px()?;
        let a_px = a_px.clamp(px, px + pw);
        let b_px = b_px.clamp(px, px + pw);
        let a = self.view.px_to_x(a_px, px, pw);
        let b = self.view.px_to_x(b_px, px, pw);
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        let (y_min, y_max) = Self::compute_mode_bounds(
            &self.series,
            self.style.mode,
            self.view.domain.x.min,
            self.view.domain.x.max,
        );
        self.view.domain.y = Domain1D::new(y_min, y_max);

        self.ensure_cached_geometry(w, h);
        if self.cached_band_paths.is_empty() {
            return;
        }

        // Draw from bottom to top. Each band gets a deterministic color.
        let outline = Stroke::new(self.style.stroke_width.max(0.8));
        for (s, path) in self.cached_band_paths.iter().enumerate() {
            let color = series_color(s);
            let fill = Brush::Solid(Color::rgba(color.r, color.g, color.b, 0.35));
            let Some(top_pts) = self.cached_top_pts.get(s) else {
                continue;
            };
            if top_pts.len() < 2 {
                continue;
            }
            ctx.fill_path(path, fill);
            ctx.stroke_polyline(top_pts, &outline, Brush::Solid(color));
        }
    }

    fn ensure_cached_geometry(&mut self, w: f32, h: f32) {
        let plot = self.plot_rect(w, h);
        let (px, py, pw, ph) = plot;
        if pw <= 0.0 || ph <= 0.0 {
            self.cached_key = None;
            self.cached_sample_xs.clear();
            self.cached_bottoms.clear();
            self.cached_tops.clear();
            self.cached_band_paths.clear();
            self.cached_top_pts.clear();
            return;
        }

        let series_n = self.series.len().min(16);
        let key = CacheKey::new(self, plot, series_n);
        if self.cached_key == Some(key) {
            return;
        }

        self.cached_sample_xs.clear();
        self.cached_band_paths.clear();

        if self.cached_top_pts.len() < series_n {
            self.cached_top_pts.resize_with(series_n, Vec::new);
        } else {
            self.cached_top_pts.truncate(series_n);
        }
        for top in &mut self.cached_top_pts {
            top.clear();
        }

        let Some(_first) = self.series.first() else {
            self.cached_key = Some(key);
            return;
        };

        // Sample at ~1-2 points per pixel for smooth fills.
        let max_samples = (pw.ceil() as usize).clamp(64, 2_000);
        self.cached_sample_xs = Self::merged_x_samples(
            &self.series[..series_n],
            self.view.domain.x.min,
            self.view.domain.x.max,
            Some(max_samples),
        );
        if self.cached_sample_xs.len() < 2 {
            self.cached_key = Some(key);
            return;
        }

        let sample_n = self.cached_sample_xs.len();
        let needed = series_n * sample_n;
        self.cached_bottoms.resize(needed, 0.0);
        self.cached_tops.resize(needed, 0.0);

        self.scratch_vals.clear();
        self.scratch_vals.resize(series_n, 0.0);

        for (k, &x) in self.cached_sample_xs.iter().enumerate() {
            let mut pos_sum = 0.0f32;
            let mut neg_sum = 0.0f32;
            for s in 0..series_n {
                let v = Self::y_at(&self.series[s], x);
                self.scratch_vals[s] = v;
                if v >= 0.0 {
                    pos_sum += v;
                } else {
                    neg_sum += v;
                }
            }

            match self.style.mode {
                StackedAreaMode::Stacked => {
                    let mut cur_pos = 0.0f32;
                    let mut cur_neg = 0.0f32;
                    for s in 0..series_n {
                        let v = self.scratch_vals[s];
                        if v >= 0.0 {
                            self.cached_bottoms[s * sample_n + k] = cur_pos;
                            cur_pos += v;
                            self.cached_tops[s * sample_n + k] = cur_pos;
                        } else {
                            self.cached_bottoms[s * sample_n + k] = cur_neg;
                            cur_neg += v;
                            self.cached_tops[s * sample_n + k] = cur_neg;
                        }
                    }
                }
                StackedAreaMode::Streamgraph => {
                    let baseline = -0.5 * (pos_sum + neg_sum);
                    let mut cur = baseline;
                    for s in 0..series_n {
                        self.cached_bottoms[s * sample_n + k] = cur;
                        cur += self.scratch_vals[s];
                        self.cached_tops[s * sample_n + k] = cur;
                    }
                }
            }
        }

        self.cached_band_paths.reserve(series_n);
        for s in 0..series_n {
            let top_pts = &mut self.cached_top_pts[s];
            top_pts.reserve(sample_n);

            let mut path: Option<blinc_core::Path> = None;
            for (k, &x) in self.cached_sample_xs.iter().enumerate() {
                let y = self.cached_tops[s * sample_n + k];
                let p = self.view.data_to_px(Point::new(x, y), px, py, pw, ph);
                top_pts.push(p);
                path = Some(match path {
                    None => blinc_core::Path::new().move_to(p.x, p.y),
                    Some(prev) => prev.line_to(p.x, p.y),
                });
            }

            let Some(mut path) = path else {
                continue;
            };
            for (k, &x) in self.cached_sample_xs.iter().enumerate().rev() {
                let y = self.cached_bottoms[s * sample_n + k];
                let p = self.view.data_to_px(Point::new(x, y), px, py, pw, ph);
                path = path.line_to(p.x, p.y);
            }
            path = path.close();
            self.cached_band_paths.push(path);
        }

        self.cached_key = Some(key);
    }

    pub fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        if let Some((a, b)) = self.brush_x.range_px() {
            let x = a.clamp(px, px + pw);
            let w = (b - a).abs().max(1.0);
            ctx.fill_rect(
                Rect::new(x.min(px + pw), py, w.min(pw), ph),
                0.0.into(),
                Brush::Solid(Color::rgba(0.35, 0.65, 1.0, 0.10)),
            );
        }

        if let Some(cx) = self.crosshair_x {
            let x = cx.clamp(px, px + pw);
            ctx.fill_rect(
                Rect::new(x, py, 1.0, ph),
                0.0.into(),
                Brush::Solid(self.style.crosshair),
            );
        }

        if let Some(x) = self.hover_x {
            let text = format!("x={:.3}", x);
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }
    }
}

impl InteractiveXChartModel for StackedAreaChartModel {
    fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        StackedAreaChartModel::on_mouse_move(self, local_x, local_y, w, h);
    }

    fn on_mouse_down(&mut self, brush_modifier: bool, local_x: f32, w: f32, h: f32) {
        StackedAreaChartModel::on_mouse_down(self, brush_modifier, local_x, w, h);
    }

    fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) {
        StackedAreaChartModel::on_scroll(self, delta_y, cursor_x_px, w, h);
    }

    fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) {
        StackedAreaChartModel::on_pinch(self, scale_delta, cursor_x_px, w, h);
    }

    fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        StackedAreaChartModel::on_drag_pan_total(self, drag_total_dx, w, h);
    }

    fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        StackedAreaChartModel::on_drag_brush_x_total(self, drag_total_dx, w, h);
    }

    fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        StackedAreaChartModel::on_mouse_up_finish_brush_x(self, w, h)
    }

    fn on_drag_end(&mut self) {
        StackedAreaChartModel::on_drag_end(self);
    }

    fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        StackedAreaChartModel::render_plot(self, ctx, w, h);
    }

    fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        StackedAreaChartModel::render_overlay(self, ctx, w, h);
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    fn view(&self) -> &ChartView {
        &self.view
    }

    fn view_mut(&mut self) -> &mut ChartView {
        &mut self.view
    }

    fn crosshair_x_mut(&mut self) -> &mut Option<f32> {
        &mut self.crosshair_x
    }

    fn is_brushing(&self) -> bool {
        self.brush_x.is_active()
    }
}

#[derive(Clone)]
pub struct StackedAreaChartHandle(pub Arc<Mutex<StackedAreaChartModel>>);

impl StackedAreaChartHandle {
    pub fn new(model: StackedAreaChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn stacked_area_chart(handle: StackedAreaChartHandle) -> impl ElementBuilder {
    stacked_area_chart_with_bindings(handle, crate::ChartInputBindings::default())
}

pub fn stacked_area_chart_with_bindings(
    handle: StackedAreaChartHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::x_chart(handle.0, bindings)
}

pub fn linked_stacked_area_chart(
    handle: StackedAreaChartHandle,
    link: ChartLinkHandle,
) -> impl ElementBuilder {
    linked_stacked_area_chart_with_bindings(handle, link, crate::ChartInputBindings::default())
}

pub fn linked_stacked_area_chart_with_bindings(
    handle: StackedAreaChartHandle,
    link: ChartLinkHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::linked_x_chart(handle.0, link, bindings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use blinc_core::{RecordingContext, Size};

    #[test]
    fn new_accepts_misaligned_x_samples() {
        let a = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 1.0]).unwrap();
        let b = TimeSeriesF32::new(vec![0.5, 1.5, 2.5], vec![0.5, 1.5, 0.5]).unwrap();
        let model = StackedAreaChartModel::new(vec![a, b]).unwrap();
        assert_eq!(model.view.domain.x.min, 0.0);
        assert_eq!(model.view.domain.x.max, 2.5);
        assert!(model.view.domain.y.max > model.view.domain.y.min);
    }

    #[test]
    fn render_plot_handles_misaligned_series_without_panic() {
        let a = TimeSeriesF32::new(vec![0.0, 1.0, 2.0, 3.0], vec![1.0, 2.0, 1.0, 0.5]).unwrap();
        let b = TimeSeriesF32::new(vec![0.5, 1.5, 2.5], vec![0.8, 1.4, 0.7]).unwrap();
        let c = TimeSeriesF32::new(vec![0.25, 2.25, 3.25], vec![0.4, 0.9, 0.6]).unwrap();
        let mut model = StackedAreaChartModel::new(vec![a, b, c]).unwrap();
        let mut ctx = RecordingContext::new(Size::new(360.0, 220.0));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            model.render_plot(&mut ctx, 360.0, 220.0);
        }));
        assert!(result.is_ok());
        assert!(model.cached_sample_xs.len() >= 2);
        assert!(!model.cached_band_paths.is_empty());
    }

    #[test]
    fn stacked_mode_supports_negative_values() {
        let a = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![2.0, -1.5, 1.0]).unwrap();
        let b = TimeSeriesF32::new(vec![0.5, 1.5, 2.5], vec![-0.8, 1.2, -1.3]).unwrap();
        let mut model = StackedAreaChartModel::new(vec![a, b]).unwrap();
        model.style.mode = StackedAreaMode::Stacked;

        let mut ctx = RecordingContext::new(Size::new(360.0, 220.0));
        model.render_plot(&mut ctx, 360.0, 220.0);
        assert!(model.view.domain.y.min < 0.0);
        assert!(model.view.domain.y.max > 0.0);
    }

    #[test]
    fn streamgraph_mode_supports_negative_values() {
        let a = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, -2.0, 0.8]).unwrap();
        let b = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![-0.5, 1.6, -1.2]).unwrap();
        let mut model = StackedAreaChartModel::new(vec![a, b]).unwrap();
        model.style.mode = StackedAreaMode::Streamgraph;

        let mut ctx = RecordingContext::new(Size::new(360.0, 220.0));
        model.render_plot(&mut ctx, 360.0, 220.0);
        assert!(model.view.domain.y.min < 0.0);
        assert!(model.view.domain.y.max > 0.0);
    }
}
