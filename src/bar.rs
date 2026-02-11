use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, TextStyle};
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::common::{draw_grid, fill_bg};
use crate::link::ChartLinkHandle;
use crate::time_series::TimeSeriesF32;
use crate::view::{ChartView, Domain1D, Domain2D};
use crate::xy_stack::InteractiveXChartModel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BinKey {
    x_min: u32,
    x_max: u32,
    plot_w: u32,
    plot_h: u32,
    series_n: u32,
}

impl BinKey {
    fn new(x_min: f32, x_max: f32, plot_w: f32, plot_h: f32, series_n: usize) -> Self {
        Self {
            x_min: x_min.to_bits(),
            x_max: x_max.to_bits(),
            plot_w: plot_w.to_bits(),
            plot_h: plot_h.to_bits(),
            series_n: series_n as u32,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BarChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,
    pub crosshair: Color,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,

    pub stacked: bool,
    pub bar_alpha: f32,
    pub max_series: usize,
    pub max_bins: usize,
}

impl Default for BarChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
            stacked: true,
            bar_alpha: 0.85,
            max_series: 16,
            max_bins: 20_000,
        }
    }
}

pub struct BarChartModel {
    pub series: Vec<TimeSeriesF32>,
    pub view: ChartView,
    pub style: BarChartStyle,

    pub crosshair_x: Option<f32>,
    pub hover_x: Option<f32>,

    bins: Vec<f32>, // bins_n * series_n
    counts: Vec<u32>,
    bins_n: usize,
    last_bin_key: Option<BinKey>,

    last_drag_total_x: Option<f32>,
    brush_x: BrushX,
}

impl BarChartModel {
    pub fn new(series: Vec<TimeSeriesF32>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !series.is_empty(),
            "BarChartModel requires at least 1 series"
        );
        anyhow::ensure!(
            !series
                .iter()
                .flat_map(|s| s.y.iter())
                .any(|v| v.is_finite() && *v < 0.0),
            "BarChartModel does not support negative values"
        );

        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut y_min = f32::INFINITY;
        let mut y_max_pos_sum = 0.0f32;

        for s in &series {
            let (sx0, sx1) = s.x_min_max();
            x_min = x_min.min(sx0);
            x_max = x_max.max(sx1);
            let (sy0, sy1) = s.y_min_max();
            y_min = y_min.min(sy0);
            if sy1.is_finite() {
                y_max_pos_sum += sy1.max(0.0);
            }
        }

        if !y_min.is_finite() {
            y_min = -1.0;
        }
        let mut y_max = if y_max_pos_sum.is_finite() && y_max_pos_sum > y_min {
            y_max_pos_sum
        } else {
            1.0
        };
        if y_max.partial_cmp(&y_min) != Some(std::cmp::Ordering::Greater) {
            y_max = y_min + 1.0;
        }

        let domain = Domain2D::new(Domain1D::new(x_min, x_max), Domain1D::new(y_min, y_max));
        Ok(Self {
            series,
            view: ChartView::new(domain),
            style: BarChartStyle::default(),
            crosshair_x: None,
            hover_x: None,
            bins: Vec::new(),
            counts: Vec::new(),
            bins_n: 0,
            last_bin_key: None,
            last_drag_total_x: None,
            brush_x: BrushX::default(),
        })
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    fn series_color(&self, i: usize) -> Color {
        // Simple deterministic palette (good enough for now).
        let hues = [
            (0.35, 0.65, 1.0),
            (0.95, 0.55, 0.35),
            (0.40, 0.85, 0.55),
            (0.90, 0.75, 0.25),
            (0.75, 0.55, 0.95),
            (0.25, 0.80, 0.85),
        ];
        let (r, g, b) = hues[i % hues.len()];
        Color::rgba(r, g, b, self.style.bar_alpha)
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

        let series_n = self.series.len().min(self.style.max_series);
        let key = BinKey::new(
            self.view.domain.x.min,
            self.view.domain.x.max,
            pw,
            ph,
            series_n,
        );
        if self.last_bin_key == Some(key) {
            return;
        }

        let bins_n = (pw.ceil() as usize)
            .clamp(8, self.style.max_bins.max(8))
            .min(self.style.max_bins);
        self.bins_n = bins_n;
        self.bins.clear();
        self.bins.resize(bins_n * series_n, 0.0);
        self.counts.clear();
        self.counts.resize(bins_n, 0);

        let x0 = self.view.domain.x.min;
        let x1 = self.view.domain.x.max;
        let span = (x1 - x0).max(1e-12);
        let inv_span = 1.0 / span;

        for (s_idx, s) in self.series.iter().take(series_n).enumerate() {
            let i0 = s.lower_bound_x(x0).min(s.len());
            let i1 = s.upper_bound_x(x1).min(s.len());
            self.counts.fill(0);

            for i in i0..i1 {
                let x = s.x[i];
                let y = s.y[i];
                if !x.is_finite() || !y.is_finite() {
                    continue;
                }
                let t = ((x - x0) * inv_span).clamp(0.0, 0.999_999);
                let bin = (t * bins_n as f32) as usize;
                let idx = s_idx * bins_n + bin;
                self.bins[idx] += y;
                self.counts[bin] = self.counts[bin].saturating_add(1);
            }

            // Convert sum->mean to keep values bounded.
            for bin in 0..bins_n {
                let c = self.counts[bin].max(1) as f32;
                self.bins[s_idx * bins_n + bin] /= c;
            }
        }

        // Expand y domain to fit current bins (stacked sum).
        if self.style.stacked {
            let mut max_sum = 0.0f32;
            for bin in 0..bins_n {
                let mut sum = 0.0f32;
                for s_idx in 0..series_n {
                    sum += self.bins[s_idx * bins_n + bin].max(0.0);
                }
                max_sum = max_sum.max(sum);
            }
            if max_sum.is_finite() && max_sum > self.view.domain.y.min {
                self.view.domain.y.max = max_sum.max(self.view.domain.y.min + 1e-6);
            }
        }

        // Touch key so hover-only doesn't recompute bins.
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

        let series_n = self.series.len().min(self.style.max_series);
        if self.bins_n == 0 || series_n == 0 {
            return;
        }

        let bar_w = (pw / self.bins_n as f32).max(1.0);
        let baseline_y = 0.0f32;
        let baseline_px = self.view.y_to_px(baseline_y, py, ph).clamp(py, py + ph);

        for bin in 0..self.bins_n {
            let x = px + bin as f32 * (pw / self.bins_n as f32);

            if self.style.stacked {
                let mut acc = baseline_y;
                for s_idx in 0..series_n {
                    let v = self.bins[s_idx * self.bins_n + bin];
                    let y0 = acc;
                    let y1 = acc + v;
                    acc = y1;

                    let y0_px = self.view.y_to_px(y0, py, ph);
                    let y1_px = self.view.y_to_px(y1, py, ph);
                    let top = y0_px.min(y1_px);
                    let bottom = y0_px.max(y1_px);
                    let rect_h = (bottom - top).max(0.5);

                    ctx.fill_rect(
                        Rect::new(x, top, bar_w, rect_h),
                        0.0.into(),
                        Brush::Solid(self.series_color(s_idx)),
                    );
                }
            } else {
                // Grouped: split bin into N groups.
                let group_w = (bar_w / series_n as f32).max(1.0);
                for s_idx in 0..series_n {
                    let v = self.bins[s_idx * self.bins_n + bin];
                    let y_px = self.view.y_to_px(v, py, ph);
                    let top = y_px.min(baseline_px);
                    let bottom = y_px.max(baseline_px);
                    let rect_h = (bottom - top).max(0.5);
                    ctx.fill_rect(
                        Rect::new(x + s_idx as f32 * group_w, top, group_w, rect_h),
                        0.0.into(),
                        Brush::Solid(self.series_color(s_idx)),
                    );
                }
            }
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

impl InteractiveXChartModel for BarChartModel {
    fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        BarChartModel::on_mouse_move(self, local_x, local_y, w, h);
    }

    fn on_mouse_down(&mut self, brush_modifier: bool, local_x: f32, w: f32, h: f32) {
        BarChartModel::on_mouse_down(self, brush_modifier, local_x, w, h);
    }

    fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) {
        BarChartModel::on_scroll(self, delta_y, cursor_x_px, w, h);
    }

    fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) {
        BarChartModel::on_pinch(self, scale_delta, cursor_x_px, w, h);
    }

    fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        BarChartModel::on_drag_pan_total(self, drag_total_dx, w, h);
    }

    fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        BarChartModel::on_drag_brush_x_total(self, drag_total_dx, w, h);
    }

    fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        BarChartModel::on_mouse_up_finish_brush_x(self, w, h)
    }

    fn on_drag_end(&mut self) {
        BarChartModel::on_drag_end(self);
    }

    fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        BarChartModel::render_plot(self, ctx, w, h);
    }

    fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        BarChartModel::render_overlay(self, ctx, w, h);
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
pub struct BarChartHandle(pub Arc<Mutex<BarChartModel>>);

impl BarChartHandle {
    pub fn new(model: BarChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn bar_chart(handle: BarChartHandle) -> impl ElementBuilder {
    bar_chart_with_bindings(handle, crate::ChartInputBindings::default())
}

pub fn bar_chart_with_bindings(
    handle: BarChartHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::x_chart(handle.0, bindings)
}

pub fn linked_bar_chart(handle: BarChartHandle, link: ChartLinkHandle) -> impl ElementBuilder {
    linked_bar_chart_with_bindings(handle, link, crate::ChartInputBindings::default())
}

pub fn linked_bar_chart_with_bindings(
    handle: BarChartHandle,
    link: ChartLinkHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::linked_x_chart(handle.0, link, bindings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_negative_values() {
        let series = TimeSeriesF32::new(vec![0.0, 1.0], vec![1.0, -0.5]).unwrap();
        assert!(BarChartModel::new(vec![series]).is_err());
    }
}
