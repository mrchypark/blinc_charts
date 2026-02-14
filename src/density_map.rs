use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::brush::BrushRect;
use crate::common::{draw_grid, fill_bg};
use crate::polygon::{point_in_polygon, polygon_area, rect_polygon};
use crate::view::{ChartView, Domain1D, Domain2D};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BinKey {
    x_min: u32,
    x_max: u32,
    y_min: u32,
    y_max: u32,
    bins_w: u32,
    bins_h: u32,
}

impl BinKey {
    fn new(domain: Domain2D, bins_w: usize, bins_h: usize) -> Self {
        Self {
            x_min: domain.x.min.to_bits(),
            x_max: domain.x.max.to_bits(),
            y_min: domain.y.min.to_bits(),
            y_max: domain.y.max.to_bits(),
            bins_w: bins_w as u32,
            bins_h: bins_h as u32,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DensityMapChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,

    /// Budget cap for drawn cells along X.
    pub max_cells_x: usize,
    /// Budget cap for drawn cells along Y.
    pub max_cells_y: usize,

    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,

    pub max_points: usize,
}

impl Default for DensityMapChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            max_cells_x: 128,
            max_cells_y: 64,
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
            max_points: 250_000,
        }
    }
}

pub struct DensityMapChartModel {
    pub points: Vec<Point>, // data coords
    pub view: ChartView,
    pub style: DensityMapChartStyle,

    pub hover_point: Option<Point>, // data coords
    pub hover_count: Option<u32>,

    selection: Option<(Point, Point)>, // data coords (a,b)

    bins: Vec<u32>,
    bins_w: usize,
    bins_h: usize,
    bins_max: u32,
    last_bin_key: Option<BinKey>,

    last_drag_total_x: Option<f32>,
    last_drag_total_y: Option<f32>,
    brush: BrushRect,
}

impl DensityMapChartModel {
    pub fn new(points: Vec<Point>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !points.is_empty(),
            "DensityMapChartModel requires non-empty points"
        );

        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut y_min = f32::INFINITY;
        let mut y_max = f32::NEG_INFINITY;
        for p in &points {
            if p.x.is_finite() && p.y.is_finite() {
                x_min = x_min.min(p.x);
                x_max = x_max.max(p.x);
                y_min = y_min.min(p.y);
                y_max = y_max.max(p.y);
            }
        }
        if !x_min.is_finite()
            || !x_max.is_finite()
            || x_max.partial_cmp(&x_min) != Some(std::cmp::Ordering::Greater)
        {
            x_min = 0.0;
            x_max = 1.0;
        }
        if !y_min.is_finite()
            || !y_max.is_finite()
            || y_max.partial_cmp(&y_min) != Some(std::cmp::Ordering::Greater)
        {
            y_min = 0.0;
            y_max = 1.0;
        }

        let domain = Domain2D::new(Domain1D::new(x_min, x_max), Domain1D::new(y_min, y_max));
        Ok(Self {
            points,
            view: ChartView::new(domain),
            style: DensityMapChartStyle::default(),
            hover_point: None,
            hover_count: None,
            selection: None,
            bins: Vec::new(),
            bins_w: 0,
            bins_h: 0,
            bins_max: 0,
            last_bin_key: None,
            last_drag_total_x: None,
            last_drag_total_y: None,
            brush: BrushRect::default(),
        })
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.hover_point = None;
            self.hover_count = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.hover_point = None;
            self.hover_count = None;
            return;
        }
        let x = self.view.px_to_x(local_x, px, pw);
        let y = self.view.px_to_y(local_y, py, ph);
        self.hover_point = Some(Point::new(x, y));

        // If bins are ready, also compute hovered bin count.
        if self.bins_w > 0 && self.bins_h > 0 && self.view.domain.is_valid() {
            let tx = ((x - self.view.domain.x.min) / self.view.domain.x.span()).clamp(0.0, 1.0);
            let ty = ((y - self.view.domain.y.min) / self.view.domain.y.span()).clamp(0.0, 1.0);
            let ix = ((tx * self.bins_w as f32).floor() as isize).clamp(0, self.bins_w as isize - 1)
                as usize;
            let iy = ((ty * self.bins_h as f32).floor() as isize).clamp(0, self.bins_h as isize - 1)
                as usize;
            self.hover_count = self.bins.get(iy * self.bins_w + ix).copied();
        } else {
            self.hover_count = None;
        }
    }

    pub fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, cursor_y_px: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let cursor_y_px = cursor_y_px.clamp(py, py + ph);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);
        let pivot_y = self.view.px_to_y(cursor_y_px, py, ph);

        let delta_y = delta_y.clamp(-250.0, 250.0);
        let zoom = (-delta_y * self.style.scroll_zoom_factor).exp();
        self.view.domain.x.zoom_about(pivot_x, zoom);
        self.view.domain.y.zoom_about(pivot_y, zoom);
        self.view.domain.x.clamp_span_min(1e-6);
        self.view.domain.y.clamp_span_min(1e-6);
    }

    pub fn on_pinch(
        &mut self,
        scale_delta: f32,
        cursor_x_px: f32,
        cursor_y_px: f32,
        w: f32,
        h: f32,
    ) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        let cursor_x_px = cursor_x_px.clamp(px, px + pw);
        let cursor_y_px = cursor_y_px.clamp(py, py + ph);
        let pivot_x = self.view.px_to_x(cursor_x_px, px, pw);
        let pivot_y = self.view.px_to_y(cursor_y_px, py, ph);

        let zoom = scale_delta.max(self.style.pinch_zoom_min);
        self.view.domain.x.zoom_about(pivot_x, zoom);
        self.view.domain.y.zoom_about(pivot_y, zoom);
        self.view.domain.x.clamp_span_min(1e-6);
        self.view.domain.y.clamp_span_min(1e-6);
    }

    pub fn on_drag_pan_total(&mut self, drag_total_dx: f32, drag_total_dy: f32, w: f32, h: f32) {
        let (_px, _py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        let prev_x = self.last_drag_total_x.replace(drag_total_dx);
        let prev_y = self.last_drag_total_y.replace(drag_total_dy);
        let dx_px = match prev_x {
            Some(p) => drag_total_dx - p,
            None => 0.0,
        };
        let dy_px = match prev_y {
            Some(p) => drag_total_dy - p,
            None => 0.0,
        };

        let dx = -dx_px / pw * self.view.domain.x.span();
        let dy = dy_px / ph * self.view.domain.y.span();
        self.view.domain.x.pan_by(dx);
        self.view.domain.y.pan_by(dy);
    }

    pub fn on_drag_end(&mut self) {
        self.last_drag_total_x = None;
        self.last_drag_total_y = None;
    }

    pub fn on_mouse_down(&mut self, shift: bool, local_x: f32, local_y: f32, w: f32, h: f32) {
        if !shift {
            return;
        }
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        self.brush
            .begin(local_x.clamp(px, px + pw), local_y.clamp(py, py + ph));
        self.last_drag_total_x = None;
        self.last_drag_total_y = None;
    }

    pub fn on_drag_brush_total(&mut self, drag_total_dx: f32, drag_total_dy: f32, w: f32, h: f32) {
        if !self.brush.is_active() {
            return;
        }
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        let Some((anchor_x, anchor_y)) = self.brush.anchor_px() else {
            return;
        };
        // DRAG provides delta-from-start, so infer current from anchor.
        self.brush.update(
            (anchor_x + drag_total_dx).clamp(px, px + pw),
            (anchor_y + drag_total_dy).clamp(py, py + ph),
        );
    }

    pub fn on_mouse_up_finish_brush(&mut self, w: f32, h: f32) -> Option<(Point, Point)> {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.brush.cancel();
            return None;
        }
        let (x0, y0, x1, y1) = self.brush.take_final_px()?;
        let ax = self.view.px_to_x(x0.clamp(px, px + pw), px, pw);
        let bx = self.view.px_to_x(x1.clamp(px, px + pw), px, pw);
        let ay = self.view.px_to_y(y0.clamp(py, py + ph), py, ph);
        let by = self.view.px_to_y(y1.clamp(py, py + ph), py, ph);
        let a = Point::new(ax.min(bx), ay.min(by));
        let b = Point::new(ax.max(bx), ay.max(by));
        self.selection = Some((a, b));
        Some((a, b))
    }

    fn color_map(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let (r, g, b) = if t < 0.33 {
            let u = t / 0.33;
            (0.10, 0.30 + 0.50 * u, 0.95)
        } else if t < 0.66 {
            let u = (t - 0.33) / 0.33;
            (0.10 + 0.85 * u, 0.80, 0.95 - 0.75 * u)
        } else {
            let u = (t - 0.66) / 0.34;
            (0.95, 0.80 - 0.65 * u, 0.20)
        };
        Color::rgba(r, g, b, 1.0)
    }

    fn ensure_bins(&mut self, w: f32, h: f32) {
        let (_px, _py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        let min_cell_px = 2.0;
        let bins_w = (pw / min_cell_px).floor() as usize;
        let bins_h = (ph / min_cell_px).floor() as usize;
        let bins_w = bins_w.clamp(8, self.style.max_cells_x.max(8));
        let bins_h = bins_h.clamp(8, self.style.max_cells_y.max(8));

        let key = BinKey::new(self.view.domain, bins_w, bins_h);
        if self.last_bin_key == Some(key) {
            return;
        }
        self.last_bin_key = Some(key);
        self.bins_w = bins_w;
        self.bins_h = bins_h;
        self.bins.clear();
        self.bins.resize(bins_w * bins_h, 0);

        if !self.view.domain.is_valid() {
            self.bins_max = 0;
            return;
        }
        let span_x = self.view.domain.x.span();
        let span_y = self.view.domain.y.span();
        let inv_x = 1.0 / span_x.max(1e-12);
        let inv_y = 1.0 / span_y.max(1e-12);

        let max_points = self.style.max_points.max(1);
        for p in self.points.iter().take(max_points) {
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            let tx = ((p.x - self.view.domain.x.min) * inv_x).clamp(0.0, 0.999_999);
            let ty = ((p.y - self.view.domain.y.min) * inv_y).clamp(0.0, 0.999_999);
            let ix = (tx * bins_w as f32) as usize;
            let iy = (ty * bins_h as f32) as usize;
            let idx = iy * bins_w + ix;
            if let Some(v) = self.bins.get_mut(idx) {
                *v = v.saturating_add(1);
            }
        }

        self.bins_max = self.bins.iter().copied().max().unwrap_or(0);
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        self.ensure_bins(w, h);
        if self.bins_w == 0 || self.bins_h == 0 {
            return;
        }

        let cell_w = (pw / self.bins_w as f32).max(1.0);
        let cell_h = (ph / self.bins_h as f32).max(1.0);

        let maxv = self.bins_max.max(1);
        let inv_log = 1.0 / ((maxv as f32 + 1.0).ln()).max(1e-6);

        for iy in 0..self.bins_h {
            for ix in 0..self.bins_w {
                let c = self.bins[iy * self.bins_w + ix];
                if c == 0 {
                    continue;
                }
                let t = ((c as f32 + 1.0).ln()) * inv_log;
                let x = px + ix as f32 * cell_w;
                let y = py + iy as f32 * cell_h;
                ctx.fill_rect(
                    Rect::new(x, y, cell_w + 0.5, cell_h + 0.5),
                    0.0.into(),
                    Brush::Solid(self.color_map(t)),
                );
            }
        }
    }

    pub fn render_overlay(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, _pw, _ph) = self.plot_rect(w, h);
        if let Some(p) = self.hover_point {
            let text = if let Some(c) = self.hover_count {
                format!("x={:.3}  y={:.3}  count={c}", p.x, p.y)
            } else {
                format!("x={:.3}  y={:.3}", p.x, p.y)
            };
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }

        if let Some((a, b)) = self.selection {
            let poly = rect_polygon(a.x, a.y, b.x, b.y);
            let area = polygon_area(&poly);
            let contains_hover = self
                .hover_point
                .map(|p| point_in_polygon(p, &poly))
                .unwrap_or(false);
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(
                &format!(
                    "sel: ({:.2},{:.2})..({:.2},{:.2})  area={:.2}  hover_in={contains_hover}",
                    a.x, a.y, b.x, b.y, area
                ),
                Point::new(px + 6.0, py + 24.0),
                &style,
            );
        }

        if let Some((x0, y0, x1, y1)) = self.brush.rect_px() {
            let x = x0.min(x1);
            let y = y0.min(y1);
            let ww = (x1 - x0).abs().max(1.0);
            let hh = (y1 - y0).abs().max(1.0);
            ctx.fill_rect(
                Rect::new(x, y, ww, hh),
                0.0.into(),
                Brush::Solid(Color::rgba(0.35, 0.65, 1.0, 0.12)),
            );
            ctx.stroke_rect(
                Rect::new(x, y, ww, hh),
                0.0.into(),
                &Stroke::new(1.0),
                Brush::Solid(Color::rgba(0.85, 0.92, 1.0, 0.35)),
            );
        }
    }
}

#[derive(Clone)]
pub struct DensityMapChartHandle(pub Arc<Mutex<DensityMapChartModel>>);

impl DensityMapChartHandle {
    pub fn new(model: DensityMapChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn density_map_chart(handle: DensityMapChartHandle) -> impl ElementBuilder {
    let model_plot = handle.0.clone();
    let model_overlay = handle.0.clone();

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
                m.on_mouse_down(
                    e.shift,
                    e.local_x,
                    e.local_y,
                    e.bounds_width,
                    e.bounds_height,
                );
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_scroll(move |e| {
            if let Ok(mut m) = model_scroll.lock() {
                m.on_scroll(
                    e.scroll_delta_y,
                    e.local_x,
                    e.local_y,
                    e.bounds_width,
                    e.bounds_height,
                );
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_pinch(move |e| {
            if let Ok(mut m) = model_pinch.lock() {
                m.on_pinch(
                    e.pinch_scale,
                    e.local_x,
                    e.local_y,
                    e.bounds_width,
                    e.bounds_height,
                );
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_drag(move |e| {
            if let Ok(mut m) = model_drag.lock() {
                if e.shift {
                    m.on_drag_brush_total(
                        e.drag_delta_x,
                        e.drag_delta_y,
                        e.bounds_width,
                        e.bounds_height,
                    );
                } else {
                    m.on_drag_pan_total(
                        e.drag_delta_x,
                        e.drag_delta_y,
                        e.bounds_width,
                        e.bounds_height,
                    );
                }
                blinc_layout::stateful::request_redraw();
            }
        })
        .on_mouse_up(move |e| {
            if let Ok(mut m) = model_up.lock() {
                let _ = m.on_mouse_up_finish_brush(e.bounds_width, e.bounds_height);
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
                if let Ok(m) = model_overlay.lock() {
                    m.render_overlay(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full()
            .foreground(),
        )
}
