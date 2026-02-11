use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::common::{draw_grid, fill_bg};
use crate::link::ChartLinkHandle;
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};
use crate::xy_stack::InteractiveXChartModel;

fn series_color(i: usize) -> Color {
    let hues = [
        (0.35, 0.65, 1.0),
        (0.95, 0.55, 0.35),
        (0.40, 0.85, 0.55),
        (0.90, 0.75, 0.25),
        (0.75, 0.55, 0.95),
        (0.25, 0.80, 0.85),
    ];
    let (r, g, b) = hues[i % hues.len()];
    Color::rgba(r, g, b, 0.85)
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
    plot_x: u32,
    plot_y: u32,
    plot_w: u32,
    plot_h: u32,
    mode: u8,
    series_n: u32,
    total_y_max: u32,
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
            plot_x: px.to_bits(),
            plot_y: py.to_bits(),
            plot_w: pw.to_bits(),
            plot_h: ph.to_bits(),
            mode,
            series_n: series_n as u32,
            total_y_max: model.total_y_max.to_bits(),
        }
    }
}

pub struct StackedAreaChartModel {
    pub series: Vec<TimeSeriesF32>,
    pub view: ChartView,
    pub style: StackedAreaChartStyle,

    pub crosshair_x: Option<f32>,
    pub hover_x: Option<f32>,

    total_y_max: f32,

    last_drag_total_x: Option<f32>,
    brush_x: BrushX,

    // Cached geometry (local px) for plot rendering.
    cached_key: Option<CacheKey>,
    cached_samples: Vec<usize>,
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

        let first = &series[0];
        let n = first.len();
        for s in &series[1..] {
            anyhow::ensure!(s.len() == n, "all series must have the same length");
            anyhow::ensure!(
                s.x.iter().zip(first.x.iter()).all(|(a, b)| a == b),
                "all series must share identical x samples (v1 constraint)"
            );
        }

        let (x0, x1) = first.x_min_max();
        let mut y_max = 0.0f32;
        for i in 0..n {
            let mut sum = 0.0f32;
            for s in &series {
                let v = s.y[i];
                if v.is_finite() {
                    sum += v.max(0.0);
                }
            }
            y_max = y_max.max(sum);
        }
        if !y_max.is_finite() || y_max <= 0.0 {
            y_max = 1.0;
        }

        let domain = Domain2D::new(Domain1D::new(x0, x1), Domain1D::new(0.0, y_max));
        Ok(Self {
            series,
            view: ChartView::new(domain),
            style: StackedAreaChartStyle::default(),
            crosshair_x: None,
            hover_x: None,
            total_y_max: y_max,
            last_drag_total_x: None,
            brush_x: BrushX::default(),
            cached_key: None,
            cached_samples: Vec::new(),
            cached_bottoms: Vec::new(),
            cached_tops: Vec::new(),
            cached_band_paths: Vec::new(),
            cached_top_pts: Vec::new(),
            scratch_vals: Vec::new(),
        })
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

        // Update Y domain based on the chosen stacking baseline.
        let y_max = self.total_y_max.max(1e-6);
        self.view.domain.y = match self.style.mode {
            StackedAreaMode::Stacked => Domain1D::new(0.0, y_max),
            StackedAreaMode::Streamgraph => Domain1D::new(-y_max * 0.55, y_max * 0.55),
        };

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
            self.cached_samples.clear();
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

        self.cached_samples.clear();
        self.cached_band_paths.clear();

        if self.cached_top_pts.len() < series_n {
            self.cached_top_pts.resize_with(series_n, Vec::new);
        } else {
            self.cached_top_pts.truncate(series_n);
        }
        for top in &mut self.cached_top_pts {
            top.clear();
        }

        let Some(first) = self.series.first() else {
            self.cached_key = Some(key);
            return;
        };

        let i0 = first.lower_bound_x(self.view.domain.x.min);
        let i1 = first.upper_bound_x(self.view.domain.x.max);
        let i1 = i1.max(i0 + 1).min(first.len());
        if i1 <= i0 + 1 {
            self.cached_key = Some(key);
            return;
        }

        // Sample at ~1-2 points per pixel for smooth fills.
        let max_samples = (pw.ceil() as usize).clamp(64, 2_000);
        let step = ((i1 - i0) / max_samples.max(1)).max(1);

        self.cached_samples.reserve(((i1 - i0) / step).max(2) + 1);
        for i in (i0..i1).step_by(step) {
            self.cached_samples.push(i);
        }
        let last = i1 - 1;
        if self.cached_samples.last().copied() != Some(last) {
            self.cached_samples.push(last);
        }
        if self.cached_samples.len() < 2 {
            self.cached_key = Some(key);
            return;
        }

        let sample_n = self.cached_samples.len();
        let needed = series_n * sample_n;
        self.cached_bottoms.resize(needed, 0.0);
        self.cached_tops.resize(needed, 0.0);

        self.scratch_vals.clear();
        self.scratch_vals.resize(series_n, 0.0);

        for (k, &i) in self.cached_samples.iter().enumerate() {
            let mut sum = 0.0f32;
            for s in 0..series_n {
                let v = self.series[s].y[i];
                let v = if v.is_finite() { v.max(0.0) } else { 0.0 };
                self.scratch_vals[s] = v;
                sum += v;
            }

            let baseline = match self.style.mode {
                StackedAreaMode::Stacked => 0.0,
                StackedAreaMode::Streamgraph => -0.5 * sum,
            };
            let mut cur = baseline;
            for s in 0..series_n {
                self.cached_bottoms[s * sample_n + k] = cur;
                cur += self.scratch_vals[s];
                self.cached_tops[s * sample_n + k] = cur;
            }
        }

        self.cached_band_paths.reserve(series_n);
        for s in 0..series_n {
            let top_pts = &mut self.cached_top_pts[s];
            top_pts.reserve(sample_n);

            let mut path: Option<blinc_core::Path> = None;
            for (k, &i) in self.cached_samples.iter().enumerate() {
                let x = first.x[i];
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
            for (k, &i) in self.cached_samples.iter().enumerate().rev() {
                let x = first.x[i];
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
