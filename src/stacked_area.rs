use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::common::{draw_grid, fill_bg};
use crate::link::ChartLinkHandle;
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackedAreaMode {
    Stacked,
    Streamgraph,
}

impl Default for StackedAreaMode {
    fn default() -> Self {
        Self::Stacked
    }
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

pub struct StackedAreaChartModel {
    pub series: Vec<TimeSeriesF32>,
    pub view: ChartView,
    pub style: StackedAreaChartStyle,

    pub crosshair_x: Option<f32>,
    pub hover_x: Option<f32>,

    total_y_max: f32,

    last_drag_total_x: Option<f32>,
    brush_x: BrushX,
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

        let Some(first) = self.series.first() else { return };

        let i0 = first.lower_bound_x(self.view.domain.x.min);
        let i1 = first.upper_bound_x(self.view.domain.x.max);
        let i1 = i1.max(i0 + 1).min(first.len());

        // Sample at ~1-2 points per pixel for smooth fills.
        let max_samples = (pw.ceil() as usize).clamp(64, 2_000);
        let step = ((i1 - i0) / max_samples.max(1)).max(1);

        let series_n = self.series.len().min(16);
        let mut xs = Vec::new();
        xs.reserve(((i1 - i0) / step).max(2));
        for i in (i0..i1).step_by(step) {
            xs.push(i);
        }
        if xs.len() < 2 {
            return;
        }

        // Precompute stacked bottoms/tops in data coords for each sampled x.
        // tops[s][k] = y_top, bottoms[s][k] = y_bottom (data space).
        let sample_n = xs.len();
        let mut bottoms: Vec<Vec<f32>> = vec![vec![0.0; sample_n]; series_n];
        let mut tops: Vec<Vec<f32>> = vec![vec![0.0; sample_n]; series_n];

        for (k, &i) in xs.iter().enumerate() {
            let mut vals = vec![0.0f32; series_n];
            let mut sum = 0.0f32;
            for s in 0..series_n {
                let v = self.series[s].y[i];
                let v = if v.is_finite() { v.max(0.0) } else { 0.0 };
                vals[s] = v;
                sum += v;
            }
            let baseline = match self.style.mode {
                StackedAreaMode::Stacked => 0.0,
                StackedAreaMode::Streamgraph => -0.5 * sum,
            };
            let mut cur = baseline;
            for s in 0..series_n {
                bottoms[s][k] = cur;
                cur += vals[s];
                tops[s][k] = cur;
            }
        }

        // Draw from bottom to top. Each band gets a deterministic color.
        let outline = Stroke::new(self.style.stroke_width.max(0.8));
        for s in 0..series_n {
            let color = series_color(s);
            let fill = Brush::Solid(Color::rgba(color.r, color.g, color.b, 0.35));

            let mut top_pts = Vec::with_capacity(sample_n);
            let mut bot_pts = Vec::with_capacity(sample_n);
            for (k, &i) in xs.iter().enumerate() {
                let x = first.x[i];
                top_pts.push(self.view.data_to_px(Point::new(x, tops[s][k]), px, py, pw, ph));
                bot_pts.push(self.view.data_to_px(
                    Point::new(x, bottoms[s][k]),
                    px,
                    py,
                    pw,
                    ph,
                ));
            }
            if top_pts.len() < 2 {
                continue;
            }

            let mut path = blinc_core::Path::new().move_to(top_pts[0].x, top_pts[0].y);
            for p in &top_pts[1..] {
                path = path.line_to(p.x, p.y);
            }
            for p in bot_pts.iter().rev() {
                path = path.line_to(p.x, p.y);
            }
            path = path.close();
            ctx.fill_path(&path, fill);
            ctx.stroke_polyline(&top_pts, &outline, Brush::Solid(color));
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
pub struct StackedAreaChartHandle(pub Arc<Mutex<StackedAreaChartModel>>);

impl StackedAreaChartHandle {
    pub fn new(model: StackedAreaChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn stacked_area_chart(handle: StackedAreaChartHandle) -> impl ElementBuilder {
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

pub fn linked_stacked_area_chart(
    handle: StackedAreaChartHandle,
    link: ChartLinkHandle,
) -> impl ElementBuilder {
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
                if let Some(x) = m.hover_x {
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
                if let Some((a, b)) = m.on_mouse_up_finish_brush_x(e.bounds_width, e.bounds_height)
                {
                    l.set_selection_x(Some((a, b)));
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
