use std::sync::{Arc, Mutex};
use std::time::Instant;

use blinc_core::{Brush, Color, DrawContext, Path, Point, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::common::fill_bg;
use crate::format::format_compact;
use crate::interpolate::lerp_f32;
use crate::transition::ValueTransition;

#[derive(Clone, Debug)]
pub struct GaugeChartStyle {
    pub bg: Color,
    pub track: Color,
    pub fill: Color,
    pub needle: Color,
    pub text: Color,

    pub stroke_width: f32,
    pub angle_start_rad: f32,
    pub angle_end_rad: f32,
}

impl Default for GaugeChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            track: Color::rgba(1.0, 1.0, 1.0, 0.10),
            fill: Color::rgba(0.35, 0.65, 1.0, 0.85),
            needle: Color::rgba(1.0, 1.0, 1.0, 0.75),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            stroke_width: 8.0,
            // 270° gauge (like a speedometer).
            angle_start_rad: -std::f32::consts::PI * 0.75,
            angle_end_rad: std::f32::consts::PI * 0.75,
        }
    }
}

pub struct GaugeChartModel {
    pub min: f32,
    pub max: f32,
    pub value: f32,
    pub style: GaugeChartStyle,
    pub transition: Option<ValueTransition>,
    pub transition_step_sec: f32,
}

impl GaugeChartModel {
    pub fn new(min: f32, max: f32, value: f32) -> anyhow::Result<Self> {
        anyhow::ensure!(min.is_finite() && max.is_finite(), "min/max must be finite");
        anyhow::ensure!(max > min, "max must be > min");
        let value = value.clamp(min, max);
        Ok(Self {
            min,
            max,
            value,
            style: GaugeChartStyle::default(),
            transition: None,
            transition_step_sec: 1.0 / 60.0,
        })
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
        self.transition = None;
    }

    pub fn set_value_transition(&mut self, value: f32, duration_seconds: f32) {
        let target = value.clamp(self.min, self.max);
        self.transition = Some(ValueTransition::new(self.value, target, duration_seconds));
    }

    pub fn tick_transition(&mut self, dt_seconds: f32) {
        let Some(mut tr) = self.transition else {
            return;
        };
        tr.step(dt_seconds);
        self.value = tr.value().clamp(self.min, self.max);
        self.transition = if tr.is_finished() { None } else { Some(tr) };
    }

    fn t(&self) -> f32 {
        let span = (self.max - self.min).max(1e-12);
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }

    fn arc_points(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, n: usize) -> Vec<Point> {
        let mut out = Vec::with_capacity(n.max(2));
        let n = n.max(2);
        for i in 0..n {
            let t = i as f32 / (n - 1) as f32;
            let a = a0 + (a1 - a0) * t;
            out.push(Point::new(cx + r * a.cos(), cy + r * a.sin()));
        }
        out
    }

    pub fn render_plot(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);

        let cx = w * 0.5;
        let cy = h * 0.56;
        let r = (w.min(h) * 0.38).max(10.0);

        let a0 = self.style.angle_start_rad;
        let a1 = self.style.angle_end_rad;
        let t = self.t();
        let av = lerp_f32(a0, a1, t);

        let n = ((r * 0.35) as usize).clamp(16, 96);
        let track_pts = Self::arc_points(cx, cy, r, a0, a1, n);
        let fill_pts = Self::arc_points(cx, cy, r, a0, av, n.max(2));

        let stroke = Stroke::new(self.style.stroke_width);
        ctx.stroke_polyline(&track_pts, &stroke, Brush::Solid(self.style.track));
        ctx.stroke_polyline(&fill_pts, &stroke, Brush::Solid(self.style.fill));

        // Needle
        let needle_len = r * 0.92;
        let needle_w = (self.style.stroke_width * 0.18).max(2.0);
        let needle = Stroke::new(needle_w);
        ctx.stroke_polyline(
            &[
                Point::new(cx, cy),
                Point::new(cx + needle_len * av.cos(), cy + needle_len * av.sin()),
            ],
            &needle,
            Brush::Solid(self.style.needle),
        );

        // Center cap
        ctx.fill_circle(
            Point::new(cx, cy),
            (self.style.stroke_width * 0.42).max(4.0),
            Brush::Solid(Color::rgba(0.06, 0.07, 0.09, 1.0)),
        );
        ctx.stroke_circle(
            Point::new(cx, cy),
            (self.style.stroke_width * 0.42).max(4.0),
            &Stroke::new(1.0),
            Brush::Solid(Color::rgba(1.0, 1.0, 1.0, 0.18)),
        );

        let label = format_compact(self.value);
        let style = TextStyle::new(22.0).with_color(self.style.text);
        ctx.draw_text(&label, Point::new(cx - 22.0, cy + r * 0.42), &style);

        let small = TextStyle::new(12.0).with_color(Color::rgba(1.0, 1.0, 1.0, 0.55));
        ctx.draw_text(
            &format!("{:.0}", self.min),
            Point::new(cx - r * 0.95, cy + r * 0.18),
            &small,
        );
        ctx.draw_text(
            &format!("{:.0}", self.max),
            Point::new(cx + r * 0.80, cy + r * 0.18),
            &small,
        );
    }
}

#[derive(Clone)]
pub struct GaugeChartHandle(pub Arc<Mutex<GaugeChartModel>>);

impl GaugeChartHandle {
    pub fn new(model: GaugeChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn gauge_chart(handle: GaugeChartHandle) -> impl ElementBuilder {
    let model_plot = handle.0.clone();
    let last_tick = Arc::new(Mutex::new(None::<Instant>));
    let last_tick_plot = last_tick.clone();
    stack().w_full().h_full().overflow_clip().child(
        canvas(move |ctx, bounds| {
            if let Ok(mut m) = model_plot.lock() {
                let now = Instant::now();
                let dt = if let Ok(mut last) = last_tick_plot.lock() {
                    let dt = last
                        .map(|prev| (now - prev).as_secs_f32())
                        .unwrap_or(m.transition_step_sec);
                    *last = Some(now);
                    dt
                } else {
                    m.transition_step_sec
                };
                m.tick_transition(dt.clamp(1.0 / 240.0, 0.25));
                m.render_plot(ctx, bounds.width, bounds.height);
                if m.transition.is_some() {
                    blinc_layout::stateful::request_redraw();
                }
            }
        })
        .w_full()
        .h_full(),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Funnel
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct FunnelChartStyle {
    pub bg: Color,
    pub text: Color,
    pub fill: Color,
    pub stroke: Color,
}

impl Default for FunnelChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            fill: Color::rgba(0.35, 0.65, 1.0, 0.35),
            stroke: Color::rgba(1.0, 1.0, 1.0, 0.18),
        }
    }
}

pub struct FunnelChartModel {
    pub stages: Vec<(String, f32)>,
    pub style: FunnelChartStyle,
}

impl FunnelChartModel {
    pub fn new(stages: Vec<(String, f32)>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !stages.is_empty(),
            "FunnelChartModel requires non-empty stages"
        );
        anyhow::ensure!(
            stages.iter().any(|(_l, v)| v.is_finite() && *v > 0.0),
            "FunnelChartModel requires at least one positive finite value"
        );
        Ok(Self {
            stages,
            style: FunnelChartStyle::default(),
        })
    }

    pub fn render_plot(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);

        let pad = 18.0;
        let x0 = pad;
        let y0 = pad;
        let ww = (w - pad * 2.0).max(0.0);
        let hh = (h - pad * 2.0).max(0.0);
        if ww <= 0.0 || hh <= 0.0 {
            return;
        }

        let max_v = self
            .stages
            .iter()
            .map(|(_, v)| if v.is_finite() { *v } else { 0.0 })
            .fold(0.0f32, |a, b| a.max(b))
            .max(1e-6);
        let n = self.stages.len().max(1);
        let row_h = (hh / n as f32).max(1.0);

        let stroke = Stroke::new(1.0);
        for (i, (label, v)) in self.stages.iter().enumerate() {
            let v = if v.is_finite() { *v } else { 0.0 };
            let t = (v / max_v).clamp(0.0, 1.0);

            let t_next = self
                .stages
                .get(i + 1)
                .map(|(_, vv)| (*vv / max_v).clamp(0.0, 1.0))
                .unwrap_or(0.0);

            let top_w = ww * t;
            let bot_w = ww * t_next;
            let y = y0 + i as f32 * row_h;

            let cx = x0 + ww * 0.5;
            let xlt = cx - top_w * 0.5;
            let xrt = cx + top_w * 0.5;
            let xlb = cx - bot_w * 0.5;
            let xrb = cx + bot_w * 0.5;

            let path = Path::new()
                .move_to(xlt, y)
                .line_to(xrt, y)
                .line_to(xrb, y + row_h)
                .line_to(xlb, y + row_h)
                .close();

            // Slightly inset to avoid overdraw gaps due to rounding.
            ctx.fill_path(&path, Brush::Solid(self.style.fill));
            ctx.stroke_path(&path, &stroke, Brush::Solid(self.style.stroke));

            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(
                &format!("{label}  ({v:.0})"),
                Point::new(x0 + 6.0, y + 6.0),
                &style,
            );
        }
    }
}

#[derive(Clone)]
pub struct FunnelChartHandle(pub Arc<Mutex<FunnelChartModel>>);

impl FunnelChartHandle {
    pub fn new(model: FunnelChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn funnel_chart(handle: FunnelChartHandle) -> impl ElementBuilder {
    let model_plot = handle.0.clone();
    stack().w_full().h_full().overflow_clip().child(
        canvas(move |ctx, bounds| {
            if let Ok(m) = model_plot.lock() {
                m.render_plot(ctx, bounds.width, bounds.height);
            }
        })
        .w_full()
        .h_full(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_transition_reaches_target_value() {
        let mut m = GaugeChartModel::new(0.0, 100.0, 10.0).unwrap();
        m.set_value_transition(90.0, 0.5);

        for _ in 0..30 {
            m.tick_transition(1.0 / 60.0);
        }
        assert!((m.value - 90.0).abs() < 1e-3);
        assert!(m.transition.is_none());
    }
}
