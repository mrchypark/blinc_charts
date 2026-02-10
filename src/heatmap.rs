use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Point, Rect, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::common::{draw_grid, fill_bg};
use crate::view::{ChartView, Domain1D, Domain2D};

#[derive(Clone, Debug)]
pub struct HeatmapChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,

    pub max_cells_x: usize,
    pub max_cells_y: usize,
}

impl Default for HeatmapChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            // Keep total rect count <= ~8k so we don't overrun Blinc's default
            // `max_primitives` buffer (this is a demo renderer, not an image-based one).
            max_cells_x: 128,
            max_cells_y: 64,
        }
    }
}

pub struct HeatmapChartModel {
    pub grid_w: usize,
    pub grid_h: usize,
    pub values: Vec<f32>, // row-major: y*grid_w + x
    pub view: ChartView,
    pub style: HeatmapChartStyle,
}

impl HeatmapChartModel {
    pub fn new(grid_w: usize, grid_h: usize, values: Vec<f32>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            grid_w > 0 && grid_h > 0,
            "HeatmapChartModel requires non-empty grid"
        );
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
            style: HeatmapChartStyle::default(),
        })
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    fn value_range(&self) -> (f32, f32) {
        let mut vmin = f32::INFINITY;
        let mut vmax = f32::NEG_INFINITY;
        for &v in &self.values {
            if v.is_finite() {
                vmin = vmin.min(v);
                vmax = vmax.max(v);
            }
        }
        if !vmin.is_finite() || !vmax.is_finite() || !(vmax > vmin) {
            (0.0, 1.0)
        } else {
            (vmin, vmax)
        }
    }

    fn color_map(&self, t: f32) -> Color {
        // Cheap blue->cyan->yellow->red ramp (no allocations).
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
        Color::rgba(r, g, b, 0.95)
    }

    pub fn render_plot(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);

        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        let (vmin, vmax) = self.value_range();
        let inv = 1.0 / (vmax - vmin).max(1e-12);

        // Fit to screen to keep cost bounded.
        let cells_x = self.grid_w.min(self.style.max_cells_x);
        let cells_y = self.grid_h.min(self.style.max_cells_y);
        // Choose steps so the sampled grid does not exceed max_cells_{x,y}.
        // Use integer ceil-div to avoid oversampling due to float truncation.
        let step_x = ((self.grid_w + cells_x - 1) / cells_x).max(1);
        let step_y = ((self.grid_h + cells_y - 1) / cells_y).max(1);

        let sampled_w = ((self.grid_w + step_x - 1) / step_x).max(1) as f32;
        let sampled_h = ((self.grid_h + step_y - 1) / step_y).max(1) as f32;

        let cell_w = (pw / sampled_w).max(1.0);
        let cell_h = (ph / sampled_h).max(1.0);

        for gy in (0..self.grid_h).step_by(step_y) {
            for gx in (0..self.grid_w).step_by(step_x) {
                let idx = gy * self.grid_w + gx;
                let v = self.values[idx];
                if !v.is_finite() {
                    continue;
                }
                let t = (v - vmin) * inv;
                let x = px + (gx as f32 / step_x as f32) * cell_w;
                let y = py + (gy as f32 / step_y as f32) * cell_h;
                ctx.fill_rect(
                    Rect::new(x, y, cell_w + 0.5, cell_h + 0.5),
                    0.0.into(),
                    Brush::Solid(self.color_map(t)),
                );
            }
        }

        let text = format!("grid={}x{} (screen-sampled)", self.grid_w, self.grid_h);
        let style = TextStyle::new(12.0).with_color(self.style.text);
        ctx.draw_text(&text, Point::new(px + 6.0, py + 6.0), &style);
    }
}

#[derive(Clone)]
pub struct HeatmapChartHandle(pub Arc<Mutex<HeatmapChartModel>>);

impl HeatmapChartHandle {
    pub fn new(model: HeatmapChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn heatmap_chart(handle: HeatmapChartHandle) -> impl ElementBuilder {
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
