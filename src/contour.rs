use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::common::{draw_grid, fill_bg};
use crate::view::{ChartView, Domain1D, Domain2D};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SegKey {
    levels_hash: u64,
}

fn levels_hash(levels: &[f32]) -> u64 {
    // Stable, cheap hash over f32 bit patterns (not cryptographic).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &v in levels {
        h ^= v.to_bits() as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[derive(Clone, Copy, Debug, Default)]
struct BrushRect {
    active: bool,
    start_x: f32,
    start_y: f32,
    cur_x: f32,
    cur_y: f32,
}

impl BrushRect {
    fn is_active(&self) -> bool {
        self.active
    }

    fn begin(&mut self, x_px: f32, y_px: f32) {
        self.active = true;
        self.start_x = x_px;
        self.start_y = y_px;
        self.cur_x = x_px;
        self.cur_y = y_px;
    }

    fn update(&mut self, x_px: f32, y_px: f32) {
        if self.active {
            self.cur_x = x_px;
            self.cur_y = y_px;
        }
    }

    fn cancel(&mut self) {
        self.active = false;
    }

    fn rect_px(&self) -> Option<(f32, f32, f32, f32)> {
        if !self.active {
            return None;
        }
        let x0 = self.start_x.min(self.cur_x);
        let x1 = self.start_x.max(self.cur_x);
        let y0 = self.start_y.min(self.cur_y);
        let y1 = self.start_y.max(self.cur_y);
        Some((x0, y0, x1, y1))
    }

    fn take_final_px(&mut self) -> Option<(f32, f32, f32, f32)> {
        let r = self.rect_px();
        self.active = false;
        r
    }
}

#[derive(Clone, Debug)]
pub struct ContourChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,

    pub stroke: Color,
    pub stroke_width: f32,

    /// Contour levels in data value units.
    pub levels: Vec<f32>,

    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,

    /// Hard cap on total segments drawn (across all levels).
    pub max_segments: usize,
}

impl Default for ContourChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            stroke: Color::rgba(1.0, 1.0, 1.0, 0.35),
            stroke_width: 1.0,
            levels: vec![-0.5, 0.0, 0.5],
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
            max_segments: 20_000,
        }
    }
}

pub struct ContourChartModel {
    pub grid_w: usize,
    pub grid_h: usize,
    pub values: Vec<f32>, // row-major
    pub view: ChartView,
    pub style: ContourChartStyle,

    pub hover_xy: Option<Point>, // data coords (grid coords)
    pub hover_value: Option<f32>,

    segments_by_level: Vec<Vec<(Point, Point)>>, // data coords
    last_seg_key: Option<(usize, usize, SegKey)>,

    selection: Option<(Point, Point)>,
    last_drag_total_x: Option<f32>,
    last_drag_total_y: Option<f32>,
    brush: BrushRect,
}

impl ContourChartModel {
    pub fn new(grid_w: usize, grid_h: usize, values: Vec<f32>) -> anyhow::Result<Self> {
        anyhow::ensure!(grid_w > 0 && grid_h > 0, "grid must be non-empty");
        anyhow::ensure!(
            values.len() == grid_w * grid_h,
            "values must be grid_w*grid_h"
        );

        let domain = Domain2D::new(
            Domain1D::new(0.0, grid_w as f32),
            Domain1D::new(0.0, grid_h as f32),
        );
        Ok(Self {
            grid_w,
            grid_h,
            values,
            view: ChartView::new(domain),
            style: ContourChartStyle::default(),
            hover_xy: None,
            hover_value: None,
            segments_by_level: Vec::new(),
            last_seg_key: None,
            selection: None,
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
            self.hover_xy = None;
            self.hover_value = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.hover_xy = None;
            self.hover_value = None;
            return;
        }
        let x = self.view.px_to_x(local_x, px, pw);
        let y = self.view.px_to_y(local_y, py, ph);
        self.hover_xy = Some(Point::new(x, y));

        let gx = x.floor() as isize;
        let gy = y.floor() as isize;
        if gx >= 0 && gy >= 0 && (gx as usize) < self.grid_w && (gy as usize) < self.grid_h {
            let idx = gy as usize * self.grid_w + gx as usize;
            self.hover_value = self.values.get(idx).copied().filter(|v| v.is_finite());
        } else {
            self.hover_value = None;
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
        self.brush.begin(local_x.clamp(px, px + pw), local_y.clamp(py, py + ph));
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
        self.brush.update(
            (self.brush.start_x + drag_total_dx).clamp(px, px + pw),
            (self.brush.start_y + drag_total_dy).clamp(py, py + ph),
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

    fn value(&self, x: usize, y: usize) -> f32 {
        self.values[y * self.grid_w + x]
    }

    fn interp(level: f32, a: f32, b: f32) -> f32 {
        let d = b - a;
        if !d.is_finite() || d.abs() < 1e-12 {
            0.5
        } else {
            ((level - a) / d).clamp(0.0, 1.0)
        }
    }

    fn compute_segments_for_level(&self, level: f32, out: &mut Vec<(Point, Point)>) {
        out.clear();
        if self.grid_w < 2 || self.grid_h < 2 {
            return;
        }
        for y in 0..(self.grid_h - 1) {
            for x in 0..(self.grid_w - 1) {
                let v0 = self.value(x, y);
                let v1 = self.value(x + 1, y);
                let v2 = self.value(x + 1, y + 1);
                let v3 = self.value(x, y + 1);
                if !v0.is_finite() || !v1.is_finite() || !v2.is_finite() || !v3.is_finite() {
                    continue;
                }

                let mut idx = 0u8;
                if v0 > level {
                    idx |= 1;
                }
                if v1 > level {
                    idx |= 2;
                }
                if v2 > level {
                    idx |= 4;
                }
                if v3 > level {
                    idx |= 8;
                }
                if idx == 0 || idx == 15 {
                    continue;
                }

                let xf = x as f32;
                let yf = y as f32;

                let t0 = Self::interp(level, v0, v1);
                let t1 = Self::interp(level, v1, v2);
                let t2 = Self::interp(level, v2, v3);
                let t3 = Self::interp(level, v3, v0);
                let e0 = Point::new(xf + t0, yf);
                let e1 = Point::new(xf + 1.0, yf + t1);
                let e2 = Point::new(xf + 1.0 - t2, yf + 1.0);
                let e3 = Point::new(xf, yf + 1.0 - t3);

                let mut push = |a: Point, b: Point| {
                    out.push((a, b));
                };

                match idx {
                    1 => push(e3, e0),
                    2 => push(e0, e1),
                    3 => push(e3, e1),
                    4 => push(e1, e2),
                    5 => {
                        push(e3, e2);
                        push(e0, e1);
                    }
                    6 => push(e0, e2),
                    7 => push(e3, e2),
                    8 => push(e2, e3),
                    9 => push(e0, e2),
                    10 => {
                        push(e0, e3);
                        push(e1, e2);
                    }
                    11 => push(e1, e2),
                    12 => push(e1, e3),
                    13 => push(e0, e1),
                    14 => push(e3, e0),
                    _ => {}
                }
            }
        }
    }

    fn ensure_segments(&mut self) {
        let key = (
            self.grid_w,
            self.grid_h,
            SegKey {
                levels_hash: levels_hash(&self.style.levels),
            },
        );
        if self.last_seg_key == Some(key) {
            return;
        }

        self.segments_by_level.clear();
        self.segments_by_level
            .resize_with(self.style.levels.len(), Vec::new);

        // Shared scratch to avoid repeated allocations during per-level build.
        let mut scratch = Vec::new();
        for (i, &lvl) in self.style.levels.iter().enumerate() {
            self.compute_segments_for_level(lvl, &mut scratch);
            self.segments_by_level[i] = scratch.clone();
        }

        self.last_seg_key = Some(key);
    }

    fn level_color(&self, i: usize) -> Color {
        let hues = [
            (0.35, 0.65, 1.0),
            (0.95, 0.55, 0.35),
            (0.40, 0.85, 0.55),
            (0.90, 0.75, 0.25),
            (0.75, 0.55, 0.95),
            (0.25, 0.80, 0.85),
        ];
        let (r, g, b) = hues[i % hues.len()];
        Color::rgba(r, g, b, 0.65)
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        self.ensure_segments();

        let stroke = Stroke::new(self.style.stroke_width.max(0.8));
        let mut budget = self.style.max_segments.max(1);

        for (li, segs) in self.segments_by_level.iter().enumerate() {
            if budget == 0 {
                break;
            }
            let c = self.level_color(li);
            for &(a, b) in segs.iter().take(budget) {
                let pa = self.view.data_to_px(a, px, py, pw, ph);
                let pb = self.view.data_to_px(b, px, py, pw, ph);
                ctx.stroke_polyline(&[pa, pb], &stroke, Brush::Solid(c));
            }
            budget = budget.saturating_sub(segs.len().min(budget));
        }
    }

    pub fn render_overlay(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, _pw, _ph) = self.plot_rect(w, h);
        if let Some(p) = self.hover_xy {
            let text = if let Some(v) = self.hover_value {
                format!("x={:.2}  y={:.2}  v={:.3}", p.x, p.y, v)
            } else {
                format!("x={:.2}  y={:.2}", p.x, p.y)
            };
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }

        if let Some((a, b)) = self.selection {
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(
                &format!("sel: ({:.2},{:.2})..({:.2},{:.2})", a.x, a.y, b.x, b.y),
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
pub struct ContourChartHandle(pub Arc<Mutex<ContourChartModel>>);

impl ContourChartHandle {
    pub fn new(model: ContourChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn contour_chart(handle: ContourChartHandle) -> impl ElementBuilder {
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
                m.on_mouse_down(e.shift, e.local_x, e.local_y, e.bounds_width, e.bounds_height);
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
