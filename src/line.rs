use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, CornerRadius, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::ElementBuilder;

use crate::axis::{
    build_bottom_ticks, build_left_ticks, draw_bottom_axis, draw_left_axis, AxisTick,
};
use crate::brush::BrushX;
use crate::format::format_compact;
use crate::link::ChartLinkHandle;
use crate::lod::{downsample_min_max, DownsampleParams};
use crate::lod_cache::{SeriesIdentity, SeriesLodCache};
use crate::time_format::format_time_or_number;
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};
use crate::xy_stack::{ChartDamage, InteractiveXChartModel};

const LINE_LOD_MIN_BUCKET: usize = 32;
const LINE_LOD_MAX_LEVELS: usize = 8;
const LINE_LOD_MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SampleKey {
    x_min: u32,
    x_max: u32,
    y_min: u32,
    y_max: u32,
    plot_w: u32,
    plot_h: u32,
    max_points: u32,
}

impl SampleKey {
    fn new(model: &LineChartModel, plot_w: f32, plot_h: f32, max_points: usize) -> Self {
        Self {
            x_min: model.view.domain.x.min.to_bits(),
            x_max: model.view.domain.x.max.to_bits(),
            y_min: model.view.domain.y.min.to_bits(),
            y_max: model.view.domain.y.max.to_bits(),
            plot_w: plot_w.to_bits(),
            plot_h: plot_h.to_bits(),
            max_points: max_points as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AxisCacheKey {
    x_min: u32,
    x_max: u32,
    y_min: u32,
    y_max: u32,
    plot_x: u32,
    plot_y: u32,
    plot_w: u32,
    plot_h: u32,
}

impl AxisCacheKey {
    fn new(model: &LineChartModel, plot: (f32, f32, f32, f32)) -> Self {
        let (px, py, pw, ph) = plot;
        Self {
            x_min: model.view.domain.x.min.to_bits(),
            x_max: model.view.domain.x.max.to_bits(),
            y_min: model.view.domain.y.min.to_bits(),
            y_max: model.view.domain.y.max.to_bits(),
            plot_x: px.to_bits(),
            plot_y: py.to_bits(),
            plot_w: pw.to_bits(),
            plot_h: ph.to_bits(),
        }
    }
}

#[derive(Default)]
struct AxisCache {
    key: Option<AxisCacheKey>,
    bottom_ticks: Vec<AxisTick>,
    left_ticks: Vec<AxisTick>,
}

impl AxisCache {
    fn clear(&mut self) {
        self.key = None;
        self.bottom_ticks.clear();
        self.left_ticks.clear();
    }
}

/// Visual styling for the line chart.
#[derive(Clone, Debug)]
pub struct LineChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub line: Color,
    pub crosshair: Color,
    pub text: Color,
    pub stroke_width: f32,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,
}

impl Default for LineChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            line: Color::rgba(0.35, 0.65, 1.0, 1.0),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            stroke_width: 1.5,
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
        }
    }
}

/// Mutable model for an interactive line chart.
///
/// Store this behind an `Arc<Mutex<_>>` and reuse across rebuilds.
pub struct LineChartModel {
    pub series: TimeSeriesF32,
    pub view: ChartView,
    pub style: LineChartStyle,

    pub crosshair_x: Option<f32>,   // local px in plot area
    pub hover_point: Option<Point>, // data coords

    downsampled: Vec<Point>, // data coords
    points_px: Vec<Point>,   // screen coords (local)
    downsample_params: DownsampleParams,
    user_max_points: usize,
    lod_cache: SeriesLodCache,
    lod_cache_identity: SeriesIdentity,

    // Cache key for (re)sampling. Hover-only interactions should not force
    // downsampling or point transforms on every frame.
    last_sample_key: Option<SampleKey>,
    axis_cache: AxisCache,

    // EventRouter's drag_delta_x/y are "offset from drag start", not per-frame deltas.
    // Track last observed totals so we can convert to incremental deltas for panning.
    last_drag_total_x: Option<f32>,

    brush_x: BrushX,
}

impl LineChartModel {
    pub fn new(series: TimeSeriesF32) -> Self {
        let (x0, x1) = series.x_min_max();
        let (mut y0, mut y1) = series.y_min_max();
        if y1.partial_cmp(&y0) != Some(std::cmp::Ordering::Greater) {
            // Handle degenerate or invalid y-ranges (e.g. all NaN -> (0,0)).
            if y0.is_finite() && y1.is_finite() {
                y0 -= 1.0;
                y1 += 1.0;
            } else {
                y0 = -1.0;
                y1 = 1.0;
            }
        }
        let domain = Domain2D::new(Domain1D::new(x0, x1), Domain1D::new(y0, y1));
        let lod_cache = SeriesLodCache::build(
            &series,
            LINE_LOD_MIN_BUCKET,
            LINE_LOD_MAX_LEVELS,
            LINE_LOD_MAX_BYTES,
        );
        let lod_cache_identity = SeriesIdentity::new(&series);
        Self {
            series,
            view: ChartView::new(domain),
            style: LineChartStyle::default(),
            crosshair_x: None,
            hover_point: None,
            downsampled: Vec::new(),
            points_px: Vec::new(),
            downsample_params: DownsampleParams::default(),
            user_max_points: DownsampleParams::default().max_points,
            lod_cache,
            lod_cache_identity,
            last_sample_key: None,
            axis_cache: AxisCache::default(),
            last_drag_total_x: None,
            brush_x: BrushX::default(),
        }
    }

    pub fn set_downsample_max_points(&mut self, max_points: usize) {
        self.user_max_points = max_points.max(64);
        self.last_sample_key = None;
    }

    fn ensure_lod_cache_fresh(&mut self) {
        let identity = SeriesIdentity::new(&self.series);
        if self.lod_cache_identity == identity {
            return;
        }

        self.lod_cache = SeriesLodCache::build(
            &self.series,
            LINE_LOD_MIN_BUCKET,
            LINE_LOD_MAX_LEVELS,
            LINE_LOD_MAX_BYTES,
        );
        self.lod_cache_identity = identity;
        self.last_sample_key = None;
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) -> ChartDamage {
        let prev_crosshair = self.crosshair_x;
        let prev_hover = self.hover_point;
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.crosshair_x = None;
            self.hover_point = None;
            return overlay_damage(
                prev_crosshair,
                prev_hover,
                self.crosshair_x,
                self.hover_point,
            );
        }

        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.crosshair_x = None;
            self.hover_point = None;
            return overlay_damage(
                prev_crosshair,
                prev_hover,
                self.crosshair_x,
                self.hover_point,
            );
        }

        self.crosshair_x = Some(local_x);
        let x = self.view.px_to_x(local_x, px, pw);
        self.hover_point = self
            .series
            .nearest_by_x(x)
            .map(|(_i, xx, yy)| Point::new(xx, yy));
        overlay_damage(
            prev_crosshair,
            prev_hover,
            self.crosshair_x,
            self.hover_point,
        )
    }

    pub fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        let prev_domain = self.view.domain.x;
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);

        // Trackpad/mouse wheel: delta_y > 0 typically means scroll down.
        // Use exponential zoom so it feels consistent across devices.
        //
        // Note: On desktop, pixel scroll deltas are normalized in the platform layer,
        // so use a larger factor to keep zoom responsive.
        let delta_y = delta_y.clamp(-250.0, 250.0);
        let zoom = (-delta_y * self.style.scroll_zoom_factor).exp();
        self.view.domain.x.zoom_about(pivot_x, zoom);

        // Prevent collapsing to 0 span.
        self.view.domain.x.clamp_span_min(1e-6);
        plot_damage(prev_domain, self.view.domain.x)
    }

    pub fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        let prev_domain = self.view.domain.x;
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);

        // EventContext::pinch_scale is "ratio delta per update (1.0 = no change)".
        let zoom = scale_delta.max(self.style.pinch_zoom_min);
        self.view.domain.x.zoom_about(pivot_x, zoom);
        self.view.domain.x.clamp_span_min(1e-6);
        plot_damage(prev_domain, self.view.domain.x)
    }

    /// Pan using drag "total delta from start" (EventContext::drag_delta_x).
    pub fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        let prev_domain = self.view.domain.x;
        let (_px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
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
        plot_damage(prev_domain, self.view.domain.x)
    }

    pub fn on_drag_end(&mut self) {
        self.last_drag_total_x = None;
    }

    pub fn on_mouse_down(&mut self, shift: bool, local_x: f32, w: f32, h: f32) -> ChartDamage {
        let prev_range = self.brush_x.range_px();
        if !shift {
            return ChartDamage::None;
        }
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        self.brush_x.begin(local_x.clamp(px, px + pw));
        self.last_drag_total_x = None;
        if self.brush_x.range_px() != prev_range {
            ChartDamage::Overlay
        } else {
            ChartDamage::None
        }
    }

    pub fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        let prev_range = self.brush_x.range_px();
        if !self.brush_x.is_active() {
            return ChartDamage::None;
        }
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return ChartDamage::None;
        }
        // Brush updates track cursor position. DRAG provides delta-from-start, so infer current x.
        let Some(start_x) = self.brush_x.anchor_px() else {
            return ChartDamage::None;
        };
        let x = start_x + drag_total_dx;
        self.brush_x.update(x.clamp(px, px + pw));
        if self.brush_x.range_px() != prev_range {
            ChartDamage::Overlay
        } else {
            ChartDamage::None
        }
    }

    pub fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            self.brush_x.cancel();
            return None;
        }
        let (a_px, b_px) = self.brush_x.take_final_px()?;
        let a = self.view.px_to_x(a_px, px, pw);
        let b = self.view.px_to_x(b_px, px, pw);
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    fn ensure_samples(&mut self, w: f32, h: f32) {
        self.ensure_lod_cache_fresh();
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.downsampled.clear();
            self.points_px.clear();
            self.last_sample_key = None;
            return;
        }

        let max_points = (pw.ceil() as usize).saturating_mul(2).clamp(128, 200_000);
        self.downsample_params.max_points = self.user_max_points.min(max_points);
        let point_budget = self.downsample_params.max_points;
        let key = SampleKey::new(self, pw, ph, point_budget);
        if self.last_sample_key == Some(key) {
            return;
        }

        self.downsampled
            .reserve(point_budget.saturating_add(8).saturating_sub(self.downsampled.capacity()));
        let raw_visible = self
            .series
            .upper_bound_x(self.view.domain.x.max)
            .saturating_sub(self.series.lower_bound_x(self.view.domain.x.min));
        if raw_visible > point_budget.saturating_add(8) {
            self.lod_cache.query_into(
                self.view.domain.x.min,
                self.view.domain.x.max,
                point_budget,
                &mut self.downsampled,
            );
        }
        if self.downsampled.len() < 2 {
            downsample_min_max(
                &self.series,
                self.view.domain.x.min,
                self.view.domain.x.max,
                self.downsample_params,
                &mut self.downsampled,
            );
        }

        // Ensure at least 2 points for drawing.
        if self.downsampled.len() == 1 {
            self.downsampled.push(self.downsampled[0]);
        }

        // Convert to local pixel points once.
        self.points_px.clear();
        self.points_px.reserve(self.downsampled.len());
        let affine = self.view.plot_affine(px, py, pw, ph);
        for p in &self.downsampled {
            self.points_px.push(affine.map_point(*p));
        }

        self.last_sample_key = Some(key);
    }

    fn axis_ticks(&mut self, plot: (f32, f32, f32, f32)) -> (&[AxisTick], &[AxisTick]) {
        let key = AxisCacheKey::new(self, plot);
        if self.axis_cache.key != Some(key) {
            let (px, py, pw, ph) = plot;
            self.axis_cache.bottom_ticks =
                build_bottom_ticks(self.view.domain.x, px, pw, 5, format_time_or_number);
            self.axis_cache.left_ticks =
                build_left_ticks(self.view.domain.y, py, ph, 5, format_compact);
            self.axis_cache.key = Some(key);
        }

        (&self.axis_cache.bottom_ticks, &self.axis_cache.left_ticks)
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        // Background
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

        // Series (cached)
        self.ensure_samples(w, h);
        if self.points_px.len() >= 2 {
            let stroke = Stroke::new(self.style.stroke_width);
            ctx.stroke_polyline(&self.points_px, &stroke, Brush::Solid(self.style.line));
        }
    }

    pub fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.axis_cache.clear();
            return;
        }

        // In-progress brush (X-only).
        if let Some((a, b)) = self.brush_x.range_px() {
            let x = a.clamp(px, px + pw);
            let w = (b - a).abs().max(1.0);
            ctx.fill_rect(
                Rect::new(x.min(px + pw), py, w.min(pw), ph),
                0.0.into(),
                Brush::Solid(Color::rgba(0.35, 0.65, 1.0, 0.10)),
            );
        }

        // Crosshair
        if let Some(cx) = self.crosshair_x {
            let x = cx.clamp(px, px + pw);
            ctx.fill_rect(
                Rect::new(x, py, 1.0, ph),
                0.0.into(),
                Brush::Solid(self.style.crosshair),
            );
        }

        let grid = self.style.grid;
        let text = self.style.text;
        let hover_point = self.hover_point;
        let (x_ticks, y_ticks) = self.axis_ticks((px, py, pw, ph));
        draw_bottom_axis(ctx, x_ticks, px, py + ph, pw, grid, text);
        draw_left_axis(ctx, y_ticks, px, py, ph, grid, text);

        // Tooltip (simple)
        if let Some(p) = hover_point {
            let text = format!(
                "x={}  y={}",
                format_time_or_number(p.x),
                format_compact(p.y)
            );
            let style = TextStyle::new(12.0).with_color(self.style.text);
            // Anchor near top-left of plot.
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }
    }
}

impl InteractiveXChartModel for LineChartModel {
    fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let _ = LineChartModel::on_mouse_move(self, local_x, local_y, w, h);
    }

    fn mouse_move_damage(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) -> ChartDamage {
        LineChartModel::on_mouse_move(self, local_x, local_y, w, h)
    }

    fn on_mouse_down(&mut self, brush_modifier: bool, local_x: f32, w: f32, h: f32) {
        let _ = LineChartModel::on_mouse_down(self, brush_modifier, local_x, w, h);
    }

    fn mouse_down_damage(
        &mut self,
        brush_modifier: bool,
        local_x: f32,
        w: f32,
        h: f32,
    ) -> ChartDamage {
        LineChartModel::on_mouse_down(self, brush_modifier, local_x, w, h)
    }

    fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) {
        let _ = LineChartModel::on_scroll(self, delta_y, cursor_x_px, w, h);
    }

    fn scroll_damage(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        LineChartModel::on_scroll(self, delta_y, cursor_x_px, w, h)
    }

    fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) {
        let _ = LineChartModel::on_pinch(self, scale_delta, cursor_x_px, w, h);
    }

    fn pinch_damage(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) -> ChartDamage {
        LineChartModel::on_pinch(self, scale_delta, cursor_x_px, w, h)
    }

    fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        let _ = LineChartModel::on_drag_pan_total(self, drag_total_dx, w, h);
    }

    fn drag_pan_damage(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        LineChartModel::on_drag_pan_total(self, drag_total_dx, w, h)
    }

    fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        let _ = LineChartModel::on_drag_brush_x_total(self, drag_total_dx, w, h);
    }

    fn drag_brush_damage(&mut self, drag_total_dx: f32, w: f32, h: f32) -> ChartDamage {
        LineChartModel::on_drag_brush_x_total(self, drag_total_dx, w, h)
    }

    fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        LineChartModel::on_mouse_up_finish_brush_x(self, w, h)
    }

    fn on_drag_end(&mut self) {
        LineChartModel::on_drag_end(self);
    }

    fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        LineChartModel::render_plot(self, ctx, w, h);
    }

    fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        LineChartModel::render_overlay(self, ctx, w, h);
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

/// Shared handle for a line chart model.
#[derive(Clone)]
pub struct LineChartHandle(pub Arc<Mutex<LineChartModel>>);

impl LineChartHandle {
    pub fn new(model: LineChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

/// Create an interactive line chart element.
///
/// Composition:
/// - Root: `stack()` (so callers can overlay additional canvases/elements)
/// - Child 1: plot `canvas` (background layer)
/// - Child 2: overlay `canvas` (foreground layer)
///
/// Interactions (using Blinc events):
/// - `on_mouse_move`: updates crosshair + nearest-point hover
/// - `on_scroll` / `on_pinch`: zoom X about cursor
/// - `on_drag`: pan X
pub fn line_chart(handle: LineChartHandle) -> impl ElementBuilder {
    line_chart_with_bindings(handle, crate::ChartInputBindings::default())
}

pub fn line_chart_with_bindings(
    handle: LineChartHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::x_chart(handle.0, bindings)
}

/// Create a linked line chart element.
///
/// Additional behaviors:
/// - Pan/zoom is synchronized via `link.x_domain` (shared X domain).
/// - Hover is broadcast via `link.hover_x`.
/// - Selection (brush) is created with Shift+Drag and stored in `link.selection_x`.
pub fn linked_line_chart(handle: LineChartHandle, link: ChartLinkHandle) -> impl ElementBuilder {
    linked_line_chart_with_bindings(handle, link, crate::ChartInputBindings::default())
}

pub fn linked_line_chart_with_bindings(
    handle: LineChartHandle,
    link: ChartLinkHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::linked_x_chart(handle.0, link, bindings)
}

fn overlay_damage(
    prev_crosshair: Option<f32>,
    prev_hover: Option<Point>,
    next_crosshair: Option<f32>,
    next_hover: Option<Point>,
) -> ChartDamage {
    if prev_crosshair != next_crosshair || prev_hover != next_hover {
        ChartDamage::Overlay
    } else {
        ChartDamage::None
    }
}

fn plot_damage(prev_domain: Domain1D, next_domain: Domain1D) -> ChartDamage {
    if prev_domain != next_domain {
        ChartDamage::Plot
    } else {
        ChartDamage::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Domain1D;
    use crate::xy_stack::ChartDamage;

    #[test]
    fn mouse_move_inside_plot_returns_overlay_damage() {
        let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
        let mut model = LineChartModel::new(series);

        let damage = model.on_mouse_move(120.0, 40.0, 320.0, 200.0);

        assert_eq!(damage, ChartDamage::Overlay);
    }

    #[test]
    fn axis_cache_key_changes_when_domain_changes() {
        let series = TimeSeriesF32::new(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0]).unwrap();
        let mut model = LineChartModel::new(series);
        let plot = model.plot_rect(320.0, 200.0);
        let initial = AxisCacheKey::new(&model, plot);

        model.view.domain.x.pan_by(1.0);

        let updated = AxisCacheKey::new(&model, plot);
        assert_ne!(initial, updated);
    }

    #[test]
    fn ensure_samples_uses_raw_points_for_small_visible_windows() {
        let x: Vec<f32> = (0..256).map(|i| i as f32).collect();
        let y: Vec<f32> = x.iter().map(|v| v.sin()).collect();
        let series = TimeSeriesF32::new(x, y).unwrap();
        let mut model = LineChartModel::new(series);
        model.view.domain.x = Domain1D::new(100.0, 110.0);

        model.ensure_samples(320.0, 200.0);

        assert_eq!(model.downsampled.len(), 11);
        assert_eq!(model.downsampled.first().unwrap().x, 100.0);
        assert_eq!(model.downsampled.last().unwrap().x, 110.0);
    }
}
