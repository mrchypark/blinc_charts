use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Path, Point, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::common::{draw_grid, fill_bg};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PolarChartMode {
    #[default]
    Radar,
    Polar,
    Parallel,
}

#[derive(Clone, Debug)]
pub struct PolarChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,
    pub stroke: Color,

    pub mode: PolarChartMode,
    pub fill_alpha: f32,

    pub min_value: f32,
    pub max_value: f32,

    pub max_series: usize,
}

impl Default for PolarChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            stroke: Color::rgba(0.35, 0.65, 1.0, 0.85),
            mode: PolarChartMode::Radar,
            fill_alpha: 0.20,
            min_value: 0.0,
            max_value: 1.0,
            max_series: 16,
        }
    }
}

pub struct PolarChartModel {
    pub mode: PolarChartMode,
    pub dimensions: Vec<String>,
    pub series: Vec<Vec<f32>>, // series_n x dims_n
    pub style: PolarChartStyle,

    pub hover_dim: Option<usize>,
}

impl PolarChartModel {
    pub fn new_radar(dimensions: Vec<String>, series: Vec<Vec<f32>>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !dimensions.is_empty(),
            "PolarChartModel(radar) requires non-empty dimensions"
        );
        anyhow::ensure!(
            !series.is_empty(),
            "PolarChartModel(radar) requires non-empty series"
        );
        let dims_n = dimensions.len();
        anyhow::ensure!(
            series.iter().all(|s| s.len() == dims_n),
            "each series must match dimensions length"
        );
        anyhow::ensure!(
            series.iter().flatten().any(|v| v.is_finite()),
            "PolarChartModel(radar) requires at least one finite value"
        );

        Ok(Self {
            mode: PolarChartMode::Radar,
            dimensions,
            series,
            style: PolarChartStyle::default(),
            hover_dim: None,
        })
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        let left = 32.0;
        let top = 16.0;
        let right = 16.0;
        let bottom = 24.0;
        let pw = (w - left - right).max(0.0);
        let ph = (h - top - bottom).max(0.0);
        (left, top, pw, ph)
    }

    fn series_color(&self, i: usize) -> Color {
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

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.hover_dim = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.hover_dim = None;
            return;
        }

        if self.mode == PolarChartMode::Parallel {
            let dims_n = self.dimensions.len().max(1);
            let idx = if dims_n <= 1 {
                0
            } else {
                let t = ((local_x - px) / pw).clamp(0.0, 1.0);
                (t * (dims_n - 1) as f32).round() as usize
            };
            self.hover_dim = Some(idx.min(dims_n - 1));
            return;
        }

        // Hover: pick nearest dimension ray by angle around the center.
        let cx = px + pw * 0.5;
        let cy = py + ph * 0.5;
        let dx = local_x - cx;
        let dy = local_y - cy;
        if dx.abs() < 1e-3 && dy.abs() < 1e-3 {
            self.hover_dim = None;
            return;
        }
        let mut a = dy.atan2(dx);
        if a < 0.0 {
            a += std::f32::consts::TAU;
        }
        let dims_n = self.dimensions.len().max(1);
        let idx = ((a / std::f32::consts::TAU) * dims_n as f32).round() as isize % dims_n as isize;
        self.hover_dim = Some(idx.max(0) as usize);
    }

    pub fn render_plot(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        match self.mode {
            PolarChartMode::Radar => self.render_radar(ctx, px, py, pw, ph),
            PolarChartMode::Polar => self.render_radar(ctx, px, py, pw, ph),
            PolarChartMode::Parallel => self.render_parallel(ctx, px, py, pw, ph),
        }
    }

    fn render_parallel(&self, ctx: &mut dyn DrawContext, px: f32, py: f32, pw: f32, ph: f32) {
        let dims_n = self.dimensions.len().max(1);
        let stroke = Stroke::new(1.0);
        let inv = 1.0 / (self.style.max_value - self.style.min_value).max(1e-12);

        let axis_x = |i: usize| -> f32 {
            if dims_n <= 1 {
                px + pw * 0.5
            } else {
                px + (i as f32 / (dims_n - 1) as f32) * pw
            }
        };

        // Vertical axes and labels.
        for i in 0..dims_n {
            let x = axis_x(i);
            let axis_color = if self.hover_dim == Some(i) {
                Color::rgba(1.0, 1.0, 1.0, 0.30)
            } else {
                self.style.grid
            };
            ctx.stroke_polyline(
                &[Point::new(x, py), Point::new(x, py + ph)],
                &stroke,
                Brush::Solid(axis_color),
            );
            if let Some(lbl) = self.dimensions.get(i) {
                let style = TextStyle::new(11.0).with_color(self.style.text);
                ctx.draw_text(lbl, Point::new(x + 3.0, py + 2.0), &style);
            }
        }

        // Series lines.
        let max_series = self.series.len().min(self.style.max_series);
        let mut pts = Vec::with_capacity(dims_n);
        for s in 0..max_series {
            let vals = &self.series[s];
            pts.clear();
            for i in 0..dims_n {
                let v = vals.get(i).copied().unwrap_or(self.style.min_value);
                let v = if v.is_finite() {
                    v
                } else {
                    self.style.min_value
                };
                let t = ((v - self.style.min_value) * inv).clamp(0.0, 1.0);
                let x = axis_x(i);
                let y = py + (1.0 - t) * ph;
                pts.push(Point::new(x, y));
            }
            if pts.len() >= 2 {
                ctx.stroke_polyline(&pts, &Stroke::new(1.75), Brush::Solid(self.series_color(s)));
            }
        }
    }

    fn render_radar(&self, ctx: &mut dyn DrawContext, px: f32, py: f32, pw: f32, ph: f32) {
        let dims_n = self.dimensions.len().max(3);
        let cx = px + pw * 0.5;
        let cy = py + ph * 0.5;
        let r = (pw.min(ph) * 0.42).max(10.0);

        // Grid rings.
        let stroke = Stroke::new(1.0);
        for k in 1..=4 {
            let rr = r * (k as f32 / 4.0);
            let mut pts = Vec::with_capacity(dims_n + 1);
            for i in 0..=dims_n {
                let t = i as f32 / dims_n as f32;
                let a = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                pts.push(Point::new(cx + rr * a.cos(), cy + rr * a.sin()));
            }
            ctx.stroke_polyline(&pts, &stroke, Brush::Solid(self.style.grid));
        }

        // Axes + labels.
        for i in 0..dims_n {
            let t = i as f32 / dims_n as f32;
            let a = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let p0 = Point::new(cx, cy);
            let p1 = Point::new(cx + r * a.cos(), cy + r * a.sin());
            ctx.stroke_polyline(&[p0, p1], &stroke, Brush::Solid(self.style.grid));

            if let Some(lbl) = self.dimensions.get(i) {
                let style = TextStyle::new(11.0).with_color(self.style.text);
                ctx.draw_text(lbl, Point::new(p1.x + 4.0, p1.y + 2.0), &style);
            }
        }

        let inv = 1.0 / (self.style.max_value - self.style.min_value).max(1e-12);
        let max_series = self.series.len().min(self.style.max_series);
        for s in 0..max_series {
            let vals = &self.series[s];
            let mut pts = Vec::with_capacity(dims_n + 1);
            for i in 0..dims_n {
                let v = vals.get(i).copied().unwrap_or(0.0);
                let v = if v.is_finite() {
                    v
                } else {
                    self.style.min_value
                };
                let t = ((v - self.style.min_value) * inv).clamp(0.0, 1.0);
                let rr = r * t;
                let a = (i as f32 / dims_n as f32) * std::f32::consts::TAU
                    - std::f32::consts::FRAC_PI_2;
                pts.push(Point::new(cx + rr * a.cos(), cy + rr * a.sin()));
            }
            if let Some(first) = pts.first().copied() {
                pts.push(first);
            }

            if pts.len() >= 3 {
                // Fill polygon
                let mut path = Path::new().move_to(pts[0].x, pts[0].y);
                for p in &pts[1..] {
                    path = path.line_to(p.x, p.y);
                }
                path = path.close();
                let c = self.series_color(s);
                ctx.fill_path(
                    &path,
                    Brush::Solid(Color::rgba(c.r, c.g, c.b, self.style.fill_alpha)),
                );

                // Stroke
                ctx.stroke_polyline(&pts, &Stroke::new(1.75), Brush::Solid(c));
            }
        }
    }

    pub fn render_overlay(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, _pw, _ph) = self.plot_rect(w, h);
        if let Some(i) = self.hover_dim {
            if let Some(lbl) = self.dimensions.get(i) {
                let style = TextStyle::new(12.0).with_color(self.style.text);
                ctx.draw_text(
                    &format!("dim={i}  {lbl}"),
                    Point::new(px + 6.0, py + 6.0),
                    &style,
                );
            }
        }
    }
}

#[derive(Clone)]
pub struct PolarChartHandle(pub Arc<Mutex<PolarChartModel>>);

impl PolarChartHandle {
    pub fn new(model: PolarChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn polar_chart(handle: PolarChartHandle) -> impl ElementBuilder {
    let model_plot = handle.0.clone();
    let model_overlay = handle.0.clone();
    let model_move = handle.0.clone();

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

#[cfg(test)]
mod tests {
    use super::*;
    use blinc_core::{DrawCommand, RecordingContext, Size};

    #[test]
    fn parallel_mode_does_not_render_stub_label() {
        let mut model = PolarChartModel::new_radar(
            vec!["A".into(), "B".into(), "C".into()],
            vec![vec![0.2, 0.5, 0.9]],
        )
        .unwrap();
        model.mode = PolarChartMode::Parallel;

        let mut ctx = RecordingContext::new(Size::new(320.0, 220.0));
        model.render_plot(&mut ctx, 320.0, 220.0);

        let labels: Vec<&str> = ctx
            .commands()
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCommand::DrawText { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        assert!(
            !labels.contains(&"parallel (uses radar v1)"),
            "parallel mode must not render the old stub label"
        );
    }
}
