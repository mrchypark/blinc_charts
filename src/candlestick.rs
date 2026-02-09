use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Path, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::common::{draw_grid, fill_bg};
use crate::link::ChartLinkHandle;
use crate::view::{ChartView, Domain1D, Domain2D};

#[derive(Clone, Copy, Debug, Default)]
pub struct Candle {
    pub x: f32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
}

#[derive(Clone, Debug)]
pub struct CandleSeries {
    pub candles: Vec<Candle>,
}

impl CandleSeries {
    pub fn new(candles: Vec<Candle>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !candles.is_empty(),
            "CandleSeries requires non-empty candles"
        );
        // Expect x sorted (like TimeSeriesF32). Keep it cheap.
        for w in candles.windows(2) {
            anyhow::ensure!(w[0].x <= w[1].x, "candles.x must be sorted");
        }
        Ok(Self { candles })
    }

    pub fn len(&self) -> usize {
        self.candles.len()
    }

    pub fn x_min_max(&self) -> (f32, f32) {
        (
            self.candles.first().map(|c| c.x).unwrap_or(0.0),
            self.candles.last().map(|c| c.x).unwrap_or(1.0),
        )
    }

    pub fn y_min_max(&self) -> (f32, f32) {
        let mut y0 = f32::INFINITY;
        let mut y1 = f32::NEG_INFINITY;
        for c in &self.candles {
            if c.low.is_finite() {
                y0 = y0.min(c.low);
            }
            if c.high.is_finite() {
                y1 = y1.max(c.high);
            }
        }
        if !y0.is_finite() || !y1.is_finite() {
            (0.0, 1.0)
        } else {
            (y0, y1)
        }
    }

    pub fn lower_bound_x(&self, x: f32) -> usize {
        self.candles.partition_point(|c| c.x < x)
    }

    pub fn upper_bound_x(&self, x: f32) -> usize {
        self.candles.partition_point(|c| c.x <= x)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BinKey {
    x_min: u32,
    x_max: u32,
    plot_w: u32,
    plot_h: u32,
}

impl BinKey {
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
pub struct CandlestickChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub crosshair: Color,
    pub text: Color,
    pub up: Color,
    pub down: Color,
    pub wick: Color,
    pub stroke_width: f32,
    pub max_candles: usize,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,
}

impl Default for CandlestickChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            up: Color::rgba(0.40, 0.85, 0.55, 0.85),
            down: Color::rgba(0.95, 0.55, 0.35, 0.85),
            wick: Color::rgba(1.0, 1.0, 1.0, 0.30),
            stroke_width: 1.0,
            max_candles: 20_000,
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
        }
    }
}

pub struct CandlestickChartModel {
    pub series: CandleSeries,
    pub view: ChartView,
    pub style: CandlestickChartStyle,

    pub crosshair_x: Option<f32>,
    pub hover_x: Option<f32>,

    bins: Vec<Candle>,
    bins_n: usize,
    last_bin_key: Option<BinKey>,

    last_drag_total_x: Option<f32>,
    brush_x: BrushX,
}

impl CandlestickChartModel {
    pub fn new(series: CandleSeries) -> Self {
        let (x0, x1) = series.x_min_max();
        let (mut y0, mut y1) = series.y_min_max();
        if !(y1 > y0) {
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
            style: CandlestickChartStyle::default(),
            crosshair_x: None,
            hover_x: None,
            bins: Vec::new(),
            bins_n: 0,
            last_bin_key: None,
            last_drag_total_x: None,
            brush_x: BrushX::default(),
        }
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

    fn ensure_bins(&mut self, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        let key = BinKey::new(self.view.domain.x.min, self.view.domain.x.max, pw, ph);
        if self.last_bin_key == Some(key) {
            return;
        }

        let bins_n = (pw.ceil() as usize).clamp(16, self.style.max_candles);
        self.bins_n = bins_n;
        self.bins.clear();
        self.bins.resize(bins_n, Candle::default());
        let mut counts: Vec<u32> = vec![0; bins_n];

        let x0 = self.view.domain.x.min;
        let x1 = self.view.domain.x.max;
        let span = (x1 - x0).max(1e-12);
        let inv_span = 1.0 / span;

        let i0 = self.series.lower_bound_x(x0).min(self.series.len());
        let i1 = self.series.upper_bound_x(x1).min(self.series.len());

        for i in i0..i1 {
            let c = self.series.candles[i];
            if !(c.x.is_finite()
                && c.open.is_finite()
                && c.high.is_finite()
                && c.low.is_finite()
                && c.close.is_finite())
            {
                continue;
            }
            let t = ((c.x - x0) * inv_span).clamp(0.0, 0.999_999);
            let bin = (t * bins_n as f32) as usize;
            let count = counts[bin];
            if count == 0 {
                self.bins[bin] = c;
            } else {
                let b = &mut self.bins[bin];
                b.x = (b.x + c.x) * 0.5;
                b.high = b.high.max(c.high);
                b.low = b.low.min(c.low);
                b.close = c.close;
            }
            counts[bin] = count.saturating_add(1);
        }

        // Update y domain to visible highs/lows to keep view stable.
        let mut y0 = f32::INFINITY;
        let mut y1 = f32::NEG_INFINITY;
        for bin in 0..bins_n {
            if counts[bin] == 0 {
                continue;
            }
            let c = self.bins[bin];
            y0 = y0.min(c.low);
            y1 = y1.max(c.high);
        }
        if y0.is_finite() && y1.is_finite() && (y1 > y0) {
            self.view.domain.y.min = y0;
            self.view.domain.y.max = y1;
        }

        let _ = (px, py);
        self.last_bin_key = Some(key);
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);

        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        self.ensure_bins(w, h);
        if self.bins_n == 0 {
            return;
        }

        let bin_w = pw / self.bins_n as f32;
        let body_w = (bin_w * 0.70).clamp(1.0, 32.0);
        let wick_stroke = Stroke::new(self.style.stroke_width);

        for i in 0..self.bins_n {
            let c = self.bins[i];
            if !c.x.is_finite() || !c.high.is_finite() || !c.low.is_finite() {
                continue;
            }
            let x = px + (i as f32 + 0.5) * bin_w;

            // Wick
            let y_high = self.view.y_to_px(c.high, py, ph);
            let y_low = self.view.y_to_px(c.low, py, ph);
            let wick = Path::line(Point::new(x, y_high), Point::new(x, y_low));
            ctx.stroke_path(&wick, &wick_stroke, Brush::Solid(self.style.wick));

            // Body
            let y_open = self.view.y_to_px(c.open, py, ph);
            let y_close = self.view.y_to_px(c.close, py, ph);
            let top = y_open.min(y_close);
            let bottom = y_open.max(y_close);
            let rect_h = (bottom - top).max(1.0);
            let color = if c.close >= c.open {
                self.style.up
            } else {
                self.style.down
            };
            ctx.fill_rect(
                Rect::new(x - body_w * 0.5, top, body_w, rect_h),
                0.0.into(),
                Brush::Solid(color),
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
pub struct CandlestickChartHandle(pub Arc<Mutex<CandlestickChartModel>>);

impl CandlestickChartHandle {
    pub fn new(model: CandlestickChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn candlestick_chart(handle: CandlestickChartHandle) -> impl ElementBuilder {
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

pub fn linked_candlestick_chart(
    handle: CandlestickChartHandle,
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
                    m.render_overlay(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full()
            .foreground(),
        )
}
