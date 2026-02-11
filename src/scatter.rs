use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, TextStyle};
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::common::{draw_grid, fill_bg};
use crate::link::ChartLinkHandle;
use crate::lod::{downsample_min_max, DownsampleParams};
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};
use crate::xy_stack::InteractiveXChartModel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SampleKey {
    x_min: u32,
    x_max: u32,
    plot_w: u32,
    plot_h: u32,
}

impl SampleKey {
    fn new(x_min: f32, x_max: f32, plot_w: f32, plot_h: f32) -> Self {
        Self {
            x_min: x_min.to_bits(),
            x_max: x_max.to_bits(),
            plot_w: plot_w.to_bits(),
            plot_h: plot_h.to_bits(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScatterChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub points: Color,
    pub crosshair: Color,
    pub text: Color,
    pub point_radius: f32,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,
    pub max_points: usize,
}

impl Default for ScatterChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            points: Color::rgba(0.35, 0.65, 1.0, 0.55),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            point_radius: 1.5,
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
            // Each point is currently rendered as a separate GPU primitive.
            // Keep this below the default renderer primitive budget.
            max_points: 8_000,
        }
    }
}

pub struct ScatterChartModel {
    pub series: TimeSeriesF32,
    pub view: ChartView,
    pub style: ScatterChartStyle,

    pub crosshair_x: Option<f32>,
    pub hover_point: Option<Point>,

    points_px: Vec<Point>,
    downsampled: Vec<Point>,
    downsample_params: DownsampleParams,
    user_max_points: usize,
    last_sample_key: Option<SampleKey>,

    last_drag_total_x: Option<f32>,
    brush_x: BrushX,
}

impl ScatterChartModel {
    pub fn new(series: TimeSeriesF32) -> Self {
        let (x0, x1) = series.x_min_max();
        let (mut y0, mut y1) = series.y_min_max();
        if y1.partial_cmp(&y0) != Some(std::cmp::Ordering::Greater) {
            if y0.is_finite() && y1.is_finite() {
                y0 -= 1.0;
                y1 += 1.0;
            } else {
                y0 = -1.0;
                y1 = 1.0;
            }
        }
        let domain = Domain2D::new(Domain1D::new(x0, x1), Domain1D::new(y0, y1));
        Self {
            series,
            view: ChartView::new(domain),
            style: ScatterChartStyle::default(),
            crosshair_x: None,
            hover_point: None,
            points_px: Vec::new(),
            downsampled: Vec::new(),
            downsample_params: DownsampleParams::default(),
            user_max_points: DownsampleParams::default().max_points,
            last_sample_key: None,
            last_drag_total_x: None,
            brush_x: BrushX::default(),
        }
    }

    pub fn set_max_points(&mut self, max_points: usize) {
        self.user_max_points = max_points.max(256);
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.crosshair_x = None;
            self.hover_point = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.crosshair_x = None;
            self.hover_point = None;
            return;
        }
        self.crosshair_x = Some(local_x);
        let x = self.view.px_to_x(local_x, px, pw);
        self.hover_point = self
            .series
            .nearest_by_x(x)
            .map(|(_i, xx, yy)| Point::new(xx, yy));
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

    fn ensure_samples(&mut self, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        let key = SampleKey::new(self.view.domain.x.min, self.view.domain.x.max, pw, ph);
        if self.last_sample_key == Some(key) {
            return;
        }

        // For scatter, keep a hard cap (points-per-frame) for both perf and minimal resources.
        let max_points = (pw.ceil() as usize)
            .saturating_mul(4)
            .clamp(512, self.style.max_points);
        self.downsample_params.max_points = self.user_max_points.min(max_points);

        downsample_min_max(
            &self.series,
            self.view.domain.x.min,
            self.view.domain.x.max,
            self.downsample_params,
            &mut self.downsampled,
        );

        self.points_px.clear();
        self.points_px
            .reserve(self.downsampled.len().min(max_points));
        for p in &self.downsampled {
            let pp = self.view.data_to_px(*p, px, py, pw, ph);
            self.points_px.push(pp);
        }

        self.last_sample_key = Some(key);
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);

        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        self.ensure_samples(w, h);
        // Avoid `fill_circle` here: circles go through the glass primitive path and
        // quickly exceed the default `max_glass_primitives` budget. Render points as
        // tiny rects so the primitive cost stays predictable.
        let r = self.style.point_radius.max(0.5);
        let d = r * 2.0;
        for p in &self.points_px {
            ctx.fill_rect(
                Rect::new(p.x - r, p.y - r, d, d),
                0.0.into(),
                Brush::Solid(self.style.points),
            );
        }
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

        if let Some(p) = self.hover_point {
            let text = format!("x={:.3}  y={:.3}", p.x, p.y);
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }
    }
}

impl InteractiveXChartModel for ScatterChartModel {
    fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        ScatterChartModel::on_mouse_move(self, local_x, local_y, w, h);
    }

    fn on_mouse_down(&mut self, brush_modifier: bool, local_x: f32, w: f32, h: f32) {
        ScatterChartModel::on_mouse_down(self, brush_modifier, local_x, w, h);
    }

    fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) {
        ScatterChartModel::on_scroll(self, delta_y, cursor_x_px, w, h);
    }

    fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) {
        ScatterChartModel::on_pinch(self, scale_delta, cursor_x_px, w, h);
    }

    fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        ScatterChartModel::on_drag_pan_total(self, drag_total_dx, w, h);
    }

    fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        ScatterChartModel::on_drag_brush_x_total(self, drag_total_dx, w, h);
    }

    fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        ScatterChartModel::on_mouse_up_finish_brush_x(self, w, h)
    }

    fn on_drag_end(&mut self) {
        ScatterChartModel::on_drag_end(self);
    }

    fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        ScatterChartModel::render_plot(self, ctx, w, h);
    }

    fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        ScatterChartModel::render_overlay(self, ctx, w, h);
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
pub struct ScatterChartHandle(pub Arc<Mutex<ScatterChartModel>>);

impl ScatterChartHandle {
    pub fn new(model: ScatterChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn scatter_chart(handle: ScatterChartHandle) -> impl ElementBuilder {
    scatter_chart_with_bindings(handle, crate::ChartInputBindings::default())
}

pub fn scatter_chart_with_bindings(
    handle: ScatterChartHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::x_chart(handle.0, bindings)
}

pub fn linked_scatter_chart(
    handle: ScatterChartHandle,
    link: ChartLinkHandle,
) -> impl ElementBuilder {
    linked_scatter_chart_with_bindings(handle, link, crate::ChartInputBindings::default())
}

pub fn linked_scatter_chart_with_bindings(
    handle: ScatterChartHandle,
    link: ChartLinkHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::linked_x_chart(handle.0, link, bindings)
}
