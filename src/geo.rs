use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::common::{draw_grid, fill_bg};
use crate::view::{ChartView, Domain1D, Domain2D};

#[derive(Clone, Debug)]
pub struct GeoChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,
    pub stroke: Color,
    pub stroke_width: f32,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,

    /// Hard cap for total points drawn (across all shapes).
    pub max_points: usize,
}

impl Default for GeoChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            stroke: Color::rgba(0.85, 0.92, 1.0, 0.55),
            stroke_width: 1.25,
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
            max_points: 20_000,
        }
    }
}

pub struct GeoChartModel {
    pub shapes: Vec<Vec<Point>>, // data coords
    pub view: ChartView,
    pub style: GeoChartStyle,

    pub hover_point: Option<Point>, // data coords

    last_drag_total_x: Option<f32>,
    last_drag_total_y: Option<f32>,
}

impl GeoChartModel {
    pub fn new(shapes: Vec<Vec<Point>>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !shapes.is_empty(),
            "GeoChartModel requires non-empty shapes"
        );
        anyhow::ensure!(
            shapes
                .iter()
                .any(|s| s.iter().any(|p| p.x.is_finite() && p.y.is_finite())),
            "GeoChartModel requires at least one finite point"
        );

        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut y_min = f32::INFINITY;
        let mut y_max = f32::NEG_INFINITY;

        for s in &shapes {
            for &p in s {
                if p.x.is_finite() && p.y.is_finite() {
                    x_min = x_min.min(p.x);
                    x_max = x_max.max(p.x);
                    y_min = y_min.min(p.y);
                    y_max = y_max.max(p.y);
                }
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
            shapes,
            view: ChartView::new(domain),
            style: GeoChartStyle::default(),
            hover_point: None,
            last_drag_total_x: None,
            last_drag_total_y: None,
        })
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.hover_point = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.hover_point = None;
            return;
        }
        let x = self.view.px_to_x(local_x, px, pw);
        let y = self.view.px_to_y(local_y, py, ph);
        self.hover_point = Some(Point::new(x, y));
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

    pub fn render_plot(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        let stroke = Stroke::new(self.style.stroke_width);
        let mut budget = self.style.max_points.max(2);
        for shape in &self.shapes {
            if shape.len() < 2 {
                continue;
            }
            if budget < 2 {
                break;
            }

            let take_n = shape.len().min(budget);
            budget = budget.saturating_sub(take_n);

            let mut pts = Vec::with_capacity(take_n);
            for &p in shape.iter().take(take_n) {
                if !p.x.is_finite() || !p.y.is_finite() {
                    continue;
                }
                pts.push(self.view.data_to_px(p, px, py, pw, ph));
            }
            if pts.len() >= 2 {
                ctx.stroke_polyline(&pts, &stroke, Brush::Solid(self.style.stroke));
            }
        }

        let style = TextStyle::new(12.0).with_color(self.style.text);
        ctx.draw_text(
            &format!("shapes={} (pan/zoom)", self.shapes.len()),
            Point::new(px + 6.0, py + 6.0),
            &style,
        );
    }

    pub fn render_overlay(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, _pw, _ph) = self.plot_rect(w, h);
        if let Some(p) = self.hover_point {
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(
                &format!("x={:.3}  y={:.3}", p.x, p.y),
                Point::new(px + 6.0, py + 24.0),
                &style,
            );
        }
    }
}

#[derive(Clone)]
pub struct GeoChartHandle(pub Arc<Mutex<GeoChartModel>>);

impl GeoChartHandle {
    pub fn new(model: GeoChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn geo_chart(handle: GeoChartHandle) -> impl ElementBuilder {
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
                m.on_drag_pan_total(
                    e.drag_delta_x,
                    e.drag_delta_y,
                    e.bounds_width,
                    e.bounds_height,
                );
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
                if let Ok(m) = model_plot.lock() {
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
