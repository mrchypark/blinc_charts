use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::common::{draw_grid, fill_bg};
use crate::view::{ChartView, Domain1D, Domain2D};

#[derive(Clone, Debug)]
pub struct HistogramChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub bar: Color,
    pub crosshair: Color,
    pub text: Color,
    pub bins: usize,
}

impl Default for HistogramChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            bar: Color::rgba(0.35, 0.65, 1.0, 0.85),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            bins: 256,
        }
    }
}

pub struct HistogramChartModel {
    pub values: Vec<f32>,
    pub view: ChartView,
    pub style: HistogramChartStyle,

    pub crosshair_x: Option<f32>,
    pub hover_x: Option<f32>,

    hist: Vec<u32>,
    x_min: f32,
    x_max: f32,
    max_count: u32,

    last_drag_total_x: Option<f32>,
    brush_x: BrushX,
}

impl HistogramChartModel {
    pub fn new(values: Vec<f32>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !values.is_empty(),
            "HistogramChartModel requires non-empty values"
        );

        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        for &v in &values {
            if v.is_finite() {
                x_min = x_min.min(v);
                x_max = x_max.max(v);
            }
        }
        if !x_min.is_finite() || !x_max.is_finite() || !(x_max > x_min) {
            x_min = 0.0;
            x_max = 1.0;
        }

        let domain = Domain2D::new(Domain1D::new(x_min, x_max), Domain1D::new(0.0, 1.0));
        let mut m = Self {
            values,
            view: ChartView::new(domain),
            style: HistogramChartStyle::default(),
            crosshair_x: None,
            hover_x: None,
            hist: Vec::new(),
            x_min,
            x_max,
            max_count: 1,
            last_drag_total_x: None,
            brush_x: BrushX::default(),
        };
        m.recompute_hist();
        Ok(m)
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    fn recompute_hist(&mut self) {
        let bins = self.style.bins.clamp(8, 8192);
        self.hist.clear();
        self.hist.resize(bins, 0);
        let span = (self.x_max - self.x_min).max(1e-12);
        let inv_span = 1.0 / span;

        for &v in &self.values {
            if !v.is_finite() {
                continue;
            }
            let t = ((v - self.x_min) * inv_span).clamp(0.0, 0.999_999);
            let bin = (t * bins as f32) as usize;
            self.hist[bin] = self.hist[bin].saturating_add(1);
        }

        self.max_count = self.hist.iter().copied().max().unwrap_or(1).max(1);
        // Sync Y domain to counts.
        self.view.domain.y.min = 0.0;
        self.view.domain.y.max = self.max_count as f32;
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

        // Histogram is currently global (no pan/zoom recompute). Keep it simple for the demo.
        if self.hist.is_empty() {
            return;
        }

        let bar_w = (pw / self.hist.len() as f32).max(1.0);
        for (i, &c) in self.hist.iter().enumerate() {
            let x = px + i as f32 * (pw / self.hist.len() as f32);
            let y = c as f32;
            let y_px = self.view.y_to_px(y, py, ph);
            let top = y_px.min(py + ph);
            let bottom = py + ph;
            let rect_h = (bottom - top).max(0.5);
            ctx.fill_rect(
                Rect::new(x, top, bar_w, rect_h),
                0.0.into(),
                Brush::Solid(self.style.bar),
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

        if let Some(x) = self.hover_x {
            let text = format!("x={:.3}", x);
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }
    }
}

#[derive(Clone)]
pub struct HistogramChartHandle(pub Arc<Mutex<HistogramChartModel>>);

impl HistogramChartHandle {
    pub fn new(model: HistogramChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn histogram_chart(handle: HistogramChartHandle) -> impl ElementBuilder {
    let model_plot = handle.0.clone();
    let model_overlay = handle.0.clone();

    let model_move = handle.0.clone();
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
        .on_mouse_up(move |e| {
            if let Ok(mut m) = model_up.lock() {
                let _ = m.on_mouse_up_finish_brush_x(e.bounds_width, e.bounds_height);
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
