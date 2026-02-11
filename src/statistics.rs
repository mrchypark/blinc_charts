use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, Stroke, TextStyle};
use blinc_layout::ElementBuilder;

use crate::brush::BrushX;
use crate::common::{draw_grid, fill_bg};
use crate::view::{ChartView, Domain1D, Domain2D};
use crate::xy_stack::InteractiveXChartModel;

#[derive(Clone, Debug)]
pub struct StatisticsChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,
    pub accent: Color,
    pub crosshair: Color,

    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,
}

impl Default for StatisticsChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            accent: Color::rgba(0.35, 0.65, 1.0, 0.85),
            crosshair: Color::rgba(1.0, 1.0, 1.0, 0.35),
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct GroupStats {
    q1: f32,
    med: f32,
    q3: f32,
    lo: f32,
    hi: f32,
}

pub struct StatisticsChartModel {
    pub groups: Vec<Vec<f32>>,
    pub view: ChartView,
    pub style: StatisticsChartStyle,

    pub hover_group: Option<usize>,
    pub crosshair_x: Option<f32>,

    group_stats: Vec<Option<GroupStats>>,

    last_drag_total_x: Option<f32>,
    brush_x: BrushX,
}

impl StatisticsChartModel {
    pub fn new(groups: Vec<Vec<f32>>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !groups.is_empty(),
            "StatisticsChartModel requires non-empty groups"
        );
        anyhow::ensure!(
            groups.iter().any(|g| g.iter().any(|v| v.is_finite())),
            "StatisticsChartModel requires at least one finite value"
        );

        // Domain: x is group index, y is arbitrary value range (computed from data).
        let x0 = 0.0;
        let x1 = groups.len() as f32;

        let mut y_min = f32::INFINITY;
        let mut y_max = f32::NEG_INFINITY;
        for g in &groups {
            for &v in g {
                if v.is_finite() {
                    y_min = y_min.min(v);
                    y_max = y_max.max(v);
                }
            }
        }
        if !y_min.is_finite()
            || !y_max.is_finite()
            || y_max.partial_cmp(&y_min) != Some(std::cmp::Ordering::Greater)
        {
            y_min = 0.0;
            y_max = 1.0;
        }

        let domain = Domain2D::new(Domain1D::new(x0, x1), Domain1D::new(y_min, y_max));
        let mut m = Self {
            groups,
            view: ChartView::new(domain),
            style: StatisticsChartStyle::default(),
            hover_group: None,
            crosshair_x: None,
            group_stats: Vec::new(),
            last_drag_total_x: None,
            brush_x: BrushX::default(),
        };
        m.recompute_stats();
        Ok(m)
    }

    fn recompute_stats(&mut self) {
        self.group_stats.clear();
        self.group_stats.reserve(self.groups.len());
        for g in &self.groups {
            let mut vals: Vec<f32> = g.iter().copied().filter(|v| v.is_finite()).collect();
            if vals.is_empty() {
                self.group_stats.push(None);
                continue;
            }
            vals.sort_by(|a, b| a.total_cmp(b));
            let q = |p: f32| -> f32 {
                if vals.len() == 1 {
                    return vals[0];
                }
                let idx = (vals.len() - 1) as f32 * p.clamp(0.0, 1.0);
                let i0 = idx.floor() as usize;
                let i1 = idx.ceil() as usize;
                let t = idx - i0 as f32;
                let a = vals[i0];
                let b = vals[i1.min(vals.len() - 1)];
                a + (b - a) * t
            };
            let q1 = q(0.25);
            let med = q(0.50);
            let q3 = q(0.75);
            let iqr = (q3 - q1).max(0.0);
            let lo_fence = q1 - 1.5 * iqr;
            let hi_fence = q3 + 1.5 * iqr;

            let mut lo = vals[0];
            let mut hi = vals[vals.len() - 1];
            for &v in &vals {
                if v >= lo_fence {
                    lo = v;
                    break;
                }
            }
            for &v in vals.iter().rev() {
                if v <= hi_fence {
                    hi = v;
                    break;
                }
            }

            self.group_stats.push(Some(GroupStats {
                q1,
                med,
                q3,
                lo,
                hi,
            }));
        }
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.hover_group = None;
            self.crosshair_x = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.hover_group = None;
            self.crosshair_x = None;
            return;
        }
        self.crosshair_x = Some(local_x);
        let x = self.view.px_to_x(local_x, px, pw);
        let idx = x.floor() as isize;
        if idx >= 0 && (idx as usize) < self.groups.len() {
            self.hover_group = Some(idx as usize);
        } else {
            self.hover_group = None;
        }
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
        let a = self.view.px_to_x(a_px.clamp(px, px + pw), px, pw);
        let b = self.view.px_to_x(b_px.clamp(px, px + pw), px, pw);
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    pub fn render_plot(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        let span_x = self.view.domain.x.span().max(1e-6);
        let px_per_group = pw / span_x;
        let box_w = (px_per_group * 0.55).clamp(6.0, 48.0);
        let stroke = Stroke::new(1.25);

        // Only draw groups within the current X domain.
        let n = self.groups.len();
        let i0 = (self.view.domain.x.min.floor() as isize).clamp(0, n as isize) as usize;
        let i1 = (self.view.domain.x.max.ceil() as isize).clamp(0, n as isize) as usize;
        if i0 >= i1 {
            return;
        }

        for i in i0..i1 {
            let Some(st) = self.group_stats.get(i).and_then(|s| *s) else {
                continue;
            };

            let xc = self.view.x_to_px(i as f32 + 0.5, px, pw);
            let q1 = self.view.y_to_px(st.q1, py, ph);
            let q3 = self.view.y_to_px(st.q3, py, ph);
            let med = self.view.y_to_px(st.med, py, ph);
            let lo = self.view.y_to_px(st.lo, py, ph);
            let hi = self.view.y_to_px(st.hi, py, ph);

            let top = q3.min(q1);
            let bot = q3.max(q1);
            let rect = Rect::new(xc - box_w * 0.5, top, box_w, (bot - top).max(1.0));

            ctx.fill_rect(
                rect,
                6.0.into(),
                Brush::Solid(Color::rgba(
                    self.style.accent.r,
                    self.style.accent.g,
                    self.style.accent.b,
                    0.25,
                )),
            );
            ctx.stroke_rect(rect, 6.0.into(), &stroke, Brush::Solid(self.style.accent));

            // Median
            ctx.stroke_polyline(
                &[
                    Point::new(rect.x() + 2.0, med),
                    Point::new(rect.x() + rect.width() - 2.0, med),
                ],
                &Stroke::new(2.0),
                Brush::Solid(self.style.accent),
            );

            // Whiskers + caps
            let xw = xc;
            ctx.stroke_polyline(
                &[Point::new(xw, lo), Point::new(xw, top)],
                &stroke,
                Brush::Solid(self.style.accent),
            );
            ctx.stroke_polyline(
                &[Point::new(xw, bot), Point::new(xw, hi)],
                &stroke,
                Brush::Solid(self.style.accent),
            );
            let cap = (box_w * 0.4).clamp(6.0, 18.0);
            ctx.stroke_polyline(
                &[
                    Point::new(xw - cap * 0.5, lo),
                    Point::new(xw + cap * 0.5, lo),
                ],
                &stroke,
                Brush::Solid(self.style.accent),
            );
            ctx.stroke_polyline(
                &[
                    Point::new(xw - cap * 0.5, hi),
                    Point::new(xw + cap * 0.5, hi),
                ],
                &stroke,
                Brush::Solid(self.style.accent),
            );
        }
    }

    pub fn render_overlay(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
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

        if let Some(i) = self.hover_group {
            let text = if let Some(st) = self.group_stats.get(i).and_then(|s| *s) {
                format!(
                    "group={i}  q1={:.2}  med={:.2}  q3={:.2}",
                    st.q1, st.med, st.q3
                )
            } else {
                format!("group={i}")
            };
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
        }
    }
}

impl InteractiveXChartModel for StatisticsChartModel {
    fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        StatisticsChartModel::on_mouse_move(self, local_x, local_y, w, h);
    }

    fn on_mouse_down(&mut self, brush_modifier: bool, local_x: f32, w: f32, h: f32) {
        StatisticsChartModel::on_mouse_down(self, brush_modifier, local_x, w, h);
    }

    fn on_scroll(&mut self, delta_y: f32, cursor_x_px: f32, w: f32, h: f32) {
        StatisticsChartModel::on_scroll(self, delta_y, cursor_x_px, w, h);
    }

    fn on_pinch(&mut self, scale_delta: f32, cursor_x_px: f32, w: f32, h: f32) {
        StatisticsChartModel::on_pinch(self, scale_delta, cursor_x_px, w, h);
    }

    fn on_drag_pan_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        StatisticsChartModel::on_drag_pan_total(self, drag_total_dx, w, h);
    }

    fn on_drag_brush_x_total(&mut self, drag_total_dx: f32, w: f32, h: f32) {
        StatisticsChartModel::on_drag_brush_x_total(self, drag_total_dx, w, h);
    }

    fn on_mouse_up_finish_brush_x(&mut self, w: f32, h: f32) -> Option<(f32, f32)> {
        StatisticsChartModel::on_mouse_up_finish_brush_x(self, w, h)
    }

    fn on_drag_end(&mut self) {
        StatisticsChartModel::on_drag_end(self);
    }

    fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        StatisticsChartModel::render_plot(self, ctx, w, h);
    }

    fn render_overlay(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        StatisticsChartModel::render_overlay(self, ctx, w, h);
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
pub struct StatisticsChartHandle(pub Arc<Mutex<StatisticsChartModel>>);

impl StatisticsChartHandle {
    pub fn new(model: StatisticsChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn statistics_chart(handle: StatisticsChartHandle) -> impl ElementBuilder {
    statistics_chart_with_bindings(handle, crate::ChartInputBindings::default())
}

pub fn statistics_chart_with_bindings(
    handle: StatisticsChartHandle,
    bindings: crate::ChartInputBindings,
) -> impl ElementBuilder {
    crate::xy_stack::x_chart(handle.0, bindings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use blinc_core::{RecordingContext, Size};

    #[test]
    fn render_plot_does_not_panic_when_x_domain_is_outside_groups() {
        let mut model = StatisticsChartModel::new(vec![vec![1.0, 2.0, 3.0]]).unwrap();
        model.view.domain.x = Domain1D::new(10.0, 11.0);

        let mut ctx = RecordingContext::new(Size::new(320.0, 200.0));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            model.render_plot(&mut ctx, 320.0, 200.0);
        }));

        assert!(result.is_ok());
    }
}
