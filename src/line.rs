use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, CornerRadius, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::lod::{downsample_min_max, DownsampleParams};
use crate::brush::BrushX;
use crate::link::ChartLinkHandle;
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};

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

    // Cache key for (re)sampling. Hover-only interactions should not force
    // downsampling or point transforms on every frame.
    last_sample_key: Option<SampleKey>,

    // EventRouter's drag_delta_x/y are "offset from drag start", not per-frame deltas.
    // Track last observed totals so we can convert to incremental deltas for panning.
    last_drag_total_x: Option<f32>,

    brush_x: BrushX,
}

impl LineChartModel {
    pub fn new(series: TimeSeriesF32) -> Self {
        let (x0, x1) = series.x_min_max();
        let (mut y0, mut y1) = series.y_min_max();
        if !(y1 > y0) {
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
            last_sample_key: None,
            last_drag_total_x: None,
            brush_x: BrushX::default(),
        }
    }

    pub fn set_downsample_max_points(&mut self, max_points: usize) {
        self.user_max_points = max_points.max(64);
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
    }

    pub fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) {
        let (px, _py, pw, _ph) = self.plot_rect(w, h);
        if pw <= 0.0 {
            return;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);

        // EventContext::pinch_scale is "ratio delta per update (1.0 = no change)".
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
        // Brush updates track cursor position. DRAG provides delta-from-start, so infer current x.
        let Some(start_x) = self.brush_x.anchor_px() else {
            return;
        };
        let x = start_x + drag_total_dx;
        self.brush_x.update(x.clamp(px, px + pw));
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
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.downsampled.clear();
            self.points_px.clear();
            self.last_sample_key = None;
            return;
        }

        let key = SampleKey::new(self.view.domain.x.min, self.view.domain.x.max, pw, ph);
        if self.last_sample_key == Some(key) {
            return;
        }

        // Keep output bounded by pixels (2 points per pixel column is plenty).
        let max_points = (pw.ceil() as usize).saturating_mul(2).clamp(128, 200_000);
        self.downsample_params.max_points = self.user_max_points.min(max_points);

        downsample_min_max(
            &self.series,
            self.view.domain.x.min,
            self.view.domain.x.max,
            self.downsample_params,
            &mut self.downsampled,
        );

        // Ensure at least 2 points for drawing.
        if self.downsampled.len() == 1 {
            self.downsampled.push(self.downsampled[0]);
        }

        // Convert to local pixel points once.
        self.points_px.clear();
        self.points_px.reserve(self.downsampled.len());
        for p in &self.downsampled {
            self.points_px
                .push(self.view.data_to_px(*p, px, py, pw, ph));
        }

        self.last_sample_key = Some(key);
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

        // Tooltip (simple)
        if let Some(p) = self.hover_point {
            let text = format!("x={:.3}  y={:.3}", p.x, p.y);
            let style = TextStyle::new(12.0).with_color(self.style.text);
            // Anchor near top-left of plot.
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }
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
    let model_plot = handle.0.clone();
    let model_overlay = handle.0.clone();

    // Events mutate the shared model and request a redraw (no tree rebuild).
    let model_move = handle.0.clone();
    let model_scroll = handle.0.clone();
    let model_pinch = handle.0.clone();
    let model_down = handle.0.clone();
    let model_drag = handle.0.clone();
    let model_up = handle.0.clone();
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
        .on_mouse_down(move |e| {
            if let Ok(mut m) = model_down.lock() {
                m.on_mouse_down(e.shift, e.local_x, e.bounds_width, e.bounds_height);
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
                if e.shift {
                    m.on_drag_brush_x_total(e.drag_delta_x, e.bounds_width, e.bounds_height);
                } else {
                    m.on_drag_pan_total(e.drag_delta_x, e.bounds_width, e.bounds_height);
                }
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_mouse_up(move |_e| {
            if let Ok(mut m) = model_up.lock() {
                let _ = m.on_mouse_up_finish_brush_x(_e.bounds_width, _e.bounds_height);
                m.on_drag_end();
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

/// Create a linked line chart element.
///
/// Additional behaviors:
/// - Pan/zoom is synchronized via `link.x_domain` (shared X domain).
/// - Hover is broadcast via `link.hover_x`.
/// - Selection (brush) is created with Shift+Drag and stored in `link.selection_x`.
pub fn linked_line_chart(handle: LineChartHandle, link: ChartLinkHandle) -> impl ElementBuilder {
    let model_plot = handle.0.clone();
    let model_overlay = handle.0.clone();

    let model_move = handle.0.clone();
    let model_scroll = handle.0.clone();
    let model_pinch = handle.0.clone();
    let model_down = handle.0.clone();
    let model_drag = handle.0.clone();
    let model_up = handle.0.clone();
    let model_drag_end = handle.0.clone();

    let link_move = link.clone();
    let link_scroll = link.clone();
    let link_pinch = link.clone();
    let link_down = link.clone();
    let link_drag = link.clone();
    let link_up = link.clone();
    let link_plot = link.clone();
    let link_overlay = link.clone();

    stack()
        .w_full()
        .h_full()
        .overflow_clip()
        .cursor(blinc_layout::element::CursorStyle::Crosshair)
        .on_mouse_move(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_move.lock(), model_move.lock()) {
                m.view.domain.x = l.x_domain;
                m.on_mouse_move(e.local_x, e.local_y, e.bounds_width, e.bounds_height);

                // Publish hover x in domain units (or None when out of plot).
                let (px, _py, pw, _ph) = m.plot_rect(e.bounds_width, e.bounds_height);
                if pw > 0.0 && m.crosshair_x.is_some() {
                    let x = m.view.px_to_x(e.local_x.clamp(px, px + pw), px, pw);
                    l.set_hover_x(Some(x));
                } else {
                    l.set_hover_x(None);
                }
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_mouse_down(move |e| {
            if let (Ok(_l), Ok(mut m)) = (link_down.lock(), model_down.lock()) {
                m.on_mouse_down(e.shift, e.local_x, e.bounds_width, e.bounds_height);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_scroll(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_scroll.lock(), model_scroll.lock()) {
                m.view.domain.x = l.x_domain;
                m.on_scroll(e.scroll_delta_y, e.local_x, e.bounds_width, e.bounds_height);
                l.set_x_domain(m.view.domain.x);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_pinch(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_pinch.lock(), model_pinch.lock()) {
                m.view.domain.x = l.x_domain;
                m.on_pinch(e.pinch_scale, e.local_x, e.bounds_width, e.bounds_height);
                l.set_x_domain(m.view.domain.x);
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_drag(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_drag.lock(), model_drag.lock()) {
                m.view.domain.x = l.x_domain;
                if e.shift {
                    m.on_drag_brush_x_total(e.drag_delta_x, e.bounds_width, e.bounds_height);
                } else {
                    m.on_drag_pan_total(e.drag_delta_x, e.bounds_width, e.bounds_height);
                    l.set_x_domain(m.view.domain.x);
                }
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_mouse_up(move |e| {
            if let (Ok(mut l), Ok(mut m)) = (link_up.lock(), model_up.lock()) {
                m.view.domain.x = l.x_domain;
                if let Some(sel) = m.on_mouse_up_finish_brush_x(e.bounds_width, e.bounds_height) {
                    l.set_selection_x(Some(sel));
                }
                m.on_drag_end();
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
                if let (Ok(l), Ok(mut m)) = (link_plot.lock(), model_plot.lock()) {
                    m.view.domain.x = l.x_domain;
                    m.render_plot(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full(),
        )
        .child(
            canvas(move |ctx, bounds| {
                if let (Ok(l), Ok(mut m)) = (link_overlay.lock(), model_overlay.lock()) {
                    m.view.domain.x = l.x_domain;
                    // Selection highlight (committed).
                    if let Some((a, b)) = l.selection_x {
                        let (px, py, pw, ph) = m.plot_rect(bounds.width, bounds.height);
                        if pw > 0.0 && ph > 0.0 {
                            let xa = m.view.x_to_px(a, px, pw);
                            let xb = m.view.x_to_px(b, px, pw);
                            let x0 = xa.min(xb).clamp(px, px + pw);
                            let x1 = xa.max(xb).clamp(px, px + pw);
                            ctx.fill_rect(
                                Rect::new(x0, py, (x1 - x0).max(1.0), ph),
                                0.0.into(),
                                Brush::Solid(Color::rgba(1.0, 1.0, 1.0, 0.06)),
                            );
                        }
                    }

                    // Linked hover crosshair.
                    if let Some(hx) = l.hover_x {
                        let (px, _py, pw, _ph) = m.plot_rect(bounds.width, bounds.height);
                        if pw > 0.0 {
                            m.crosshair_x = Some(m.view.x_to_px(hx, px, pw));
                        }
                    }
                    m.render_overlay(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full()
            .foreground(),
        )
}
