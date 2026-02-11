use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Path, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::common::{draw_grid, fill_bg};
use crate::view::{ChartView, Domain1D, Domain2D};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkMode {
    #[default]
    Graph,
    Sankey,
    Chord,
}

#[derive(Clone, Debug)]
pub struct NetworkChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,
    pub node: Color,
    pub link: Color,
    pub scroll_zoom_factor: f32,
    pub pinch_zoom_min: f32,

    pub node_radius: f32,
    pub max_nodes: usize,
    pub max_links: usize,
}

impl Default for NetworkChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            node: Color::rgba(0.35, 0.65, 1.0, 0.85),
            link: Color::rgba(1.0, 1.0, 1.0, 0.18),
            scroll_zoom_factor: 0.02,
            pinch_zoom_min: 0.01,
            node_radius: 6.0,
            max_nodes: 256,
            max_links: 2_000,
        }
    }
}

#[derive(Clone, Debug)]
struct GraphLayout {
    node_pos: Vec<Point>, // data coords
}

pub struct NetworkChartModel {
    pub mode: NetworkMode,
    pub labels: Vec<String>,

    // Graph / Sankey links: (src, dst, weight)
    pub links: Vec<(usize, usize, f32)>,
    // Chord: square matrix weights (len == labels.len())
    pub chord_matrix: Vec<Vec<f32>>,

    pub view: ChartView,
    pub style: NetworkChartStyle,

    pub hover_node: Option<usize>,

    layout: GraphLayout,
    last_drag_total_x: Option<f32>,
    last_drag_total_y: Option<f32>,
}

impl NetworkChartModel {
    pub fn new_graph(nodes: Vec<String>, edges: Vec<(usize, usize)>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !nodes.is_empty(),
            "NetworkChartModel(graph) requires non-empty nodes"
        );

        let links: Vec<(usize, usize, f32)> = edges.into_iter().map(|(a, b)| (a, b, 1.0)).collect();
        Self::new(NetworkMode::Graph, nodes, links, Vec::new())
    }

    pub fn new_sankey(nodes: Vec<String>, links: Vec<(usize, usize, f32)>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !nodes.is_empty(),
            "NetworkChartModel(sankey) requires non-empty nodes"
        );
        anyhow::ensure!(
            !links.is_empty(),
            "NetworkChartModel(sankey) requires non-empty links"
        );
        Self::new(NetworkMode::Sankey, nodes, links, Vec::new())
    }

    pub fn new_chord(labels: Vec<String>, matrix: Vec<Vec<f32>>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !labels.is_empty(),
            "NetworkChartModel(chord) requires non-empty labels"
        );
        anyhow::ensure!(
            !matrix.is_empty(),
            "NetworkChartModel(chord) requires non-empty matrix"
        );
        anyhow::ensure!(
            matrix.len() == labels.len() && matrix.iter().all(|row| row.len() == labels.len()),
            "chord matrix must be NxN matching label count"
        );

        let mut links = Vec::new();
        for (i, row) in matrix.iter().enumerate().take(labels.len()) {
            for (j, &w) in row.iter().enumerate().take(labels.len()) {
                if w.is_finite() && w > 0.0 {
                    links.push((i, j, w));
                }
            }
        }

        // Create a simple circular layout for chord rendering.
        let (x0, x1, y0, y1) = (-1.2, 1.2, -1.2, 1.2);
        let domain = Domain2D::new(Domain1D::new(x0, x1), Domain1D::new(y0, y1));
        Ok(Self {
            mode: NetworkMode::Chord,
            labels,
            links,
            chord_matrix: matrix,
            view: ChartView::new(domain),
            style: NetworkChartStyle::default(),
            hover_node: None,
            layout: GraphLayout {
                node_pos: Self::circle_layout(64, 1.0),
            },
            last_drag_total_x: None,
            last_drag_total_y: None,
        })
    }

    fn new(
        mode: NetworkMode,
        labels: Vec<String>,
        links: Vec<(usize, usize, f32)>,
        chord_matrix: Vec<Vec<f32>>,
    ) -> anyhow::Result<Self> {
        let n = labels.len();
        anyhow::ensure!(n > 0, "NetworkChartModel requires at least 1 node");

        for &(a, b, w) in &links {
            anyhow::ensure!(a < n && b < n, "link index out of bounds");
            anyhow::ensure!(w.is_finite(), "link weight must be finite");
        }

        // Use a stable domain around the unit circle. Pan/zoom applies on both axes.
        let domain = Domain2D::new(Domain1D::new(-1.2, 1.2), Domain1D::new(-1.2, 1.2));
        Ok(Self {
            mode,
            labels,
            links,
            chord_matrix,
            view: ChartView::new(domain),
            style: NetworkChartStyle::default(),
            hover_node: None,
            layout: GraphLayout {
                node_pos: Self::circle_layout(n.min(512), 1.0),
            },
            last_drag_total_x: None,
            last_drag_total_y: None,
        })
    }

    fn circle_layout(n: usize, r: f32) -> Vec<Point> {
        let n = n.max(1);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / n as f32;
            let a = t * std::f32::consts::TAU;
            out.push(Point::new(r * a.cos(), r * a.sin()));
        }
        out
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        self.view.plot_rect(w, h)
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.hover_node = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.hover_node = None;
            return;
        }

        let hit_r = (self.style.node_radius * 1.6).max(8.0);
        match self.mode {
            NetworkMode::Graph => {
                // Find nearest node in screen space (capped).
                let mut best = None::<(usize, f32)>;
                let max_n = self
                    .labels
                    .len()
                    .min(self.style.max_nodes)
                    .min(self.layout.node_pos.len());
                for i in 0..max_n {
                    let p = self.layout.node_pos[i];
                    let sp = self.view.data_to_px(p, px, py, pw, ph);
                    let dx = sp.x - local_x;
                    let dy = sp.y - local_y;
                    let d2 = dx * dx + dy * dy;
                    if best.map(|b| d2 < b.1).unwrap_or(true) {
                        best = Some((i, d2));
                    }
                }
                self.hover_node = best.filter(|(_i, d2)| *d2 <= hit_r * hit_r).map(|(i, _)| i);
            }
            NetworkMode::Sankey => {
                // Match the render_sankey node layout exactly.
                let n = self.labels.len().min(self.style.max_nodes);
                if n == 0 {
                    self.hover_node = None;
                    return;
                }
                let cols = 3usize;
                let col_w = pw / cols as f32;
                let row_h = (ph / ((n as f32 / cols as f32).ceil().max(1.0))).max(24.0);

                let mut best = None::<(usize, f32)>;
                for i in 0..n {
                    let col = i % cols;
                    let row = i / cols;
                    let x = px + col as f32 * col_w + col_w * 0.15;
                    let y = py + row as f32 * row_h + row_h * 0.15;
                    let rw = col_w * 0.70;
                    let rh = row_h * 0.70;
                    let cx = x + rw * 0.5;
                    let cy = y + rh * 0.5;
                    let dx = cx - local_x;
                    let dy = cy - local_y;
                    let d2 = dx * dx + dy * dy;
                    if best.map(|b| d2 < b.1).unwrap_or(true) {
                        best = Some((i, d2));
                    }
                }
                self.hover_node = best.filter(|(_i, d2)| *d2 <= hit_r * hit_r).map(|(i, _)| i);
            }
            NetworkMode::Chord => {
                // Match the render_chord marker layout exactly.
                let n = self.labels.len().min(self.style.max_nodes).max(1);
                let cx = px + pw * 0.5;
                let cy = py + ph * 0.5;
                let r = (pw.min(ph) * 0.42).max(10.0);
                let node_pts = Self::circle_layout(n, r);

                let mut best = None::<(usize, f32)>;
                for (i, p) in node_pts.iter().enumerate() {
                    let sp = Point::new(cx + p.x, cy + p.y);
                    let dx = sp.x - local_x;
                    let dy = sp.y - local_y;
                    let d2 = dx * dx + dy * dy;
                    if best.map(|b| d2 < b.1).unwrap_or(true) {
                        best = Some((i, d2));
                    }
                }
                self.hover_node = best.filter(|(_i, d2)| *d2 <= hit_r * hit_r).map(|(i, _)| i);
            }
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

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        match self.mode {
            NetworkMode::Graph => self.render_graph(ctx, px, py, pw, ph),
            NetworkMode::Sankey => self.render_sankey(ctx, px, py, pw, ph),
            NetworkMode::Chord => self.render_chord(ctx, px, py, pw, ph),
        }

        let style = TextStyle::new(12.0).with_color(self.style.text);
        let label = match self.mode {
            NetworkMode::Graph => "graph",
            NetworkMode::Sankey => "sankey",
            NetworkMode::Chord => "chord",
        };
        ctx.draw_text(label, Point::new(px + 6.0, py + 6.0), &style);
    }

    fn render_graph(&self, ctx: &mut dyn DrawContext, px: f32, py: f32, pw: f32, ph: f32) {
        let n = self
            .labels
            .len()
            .min(self.style.max_nodes)
            .min(self.layout.node_pos.len());
        let links_n = self.links.len().min(self.style.max_links);

        let link_stroke = Stroke::new(1.0);
        for &(a, b, _w) in self.links.iter().take(links_n) {
            if a >= n || b >= n {
                continue;
            }
            let pa = self
                .view
                .data_to_px(self.layout.node_pos[a], px, py, pw, ph);
            let pb = self
                .view
                .data_to_px(self.layout.node_pos[b], px, py, pw, ph);
            ctx.stroke_polyline(&[pa, pb], &link_stroke, Brush::Solid(self.style.link));
        }

        let node_stroke = Stroke::new(1.0);
        for i in 0..n {
            let p = self
                .view
                .data_to_px(self.layout.node_pos[i], px, py, pw, ph);
            let r = self.style.node_radius.max(2.0);
            ctx.fill_circle(p, r, Brush::Solid(self.style.node));
            ctx.stroke_circle(
                p,
                r,
                &node_stroke,
                Brush::Solid(Color::rgba(0.0, 0.0, 0.0, 0.25)),
            );
        }
    }

    fn render_sankey(&self, ctx: &mut dyn DrawContext, px: f32, py: f32, pw: f32, ph: f32) {
        // Deterministic, simple sankey: place nodes by index in 3 columns.
        let n = self.labels.len().min(self.style.max_nodes);
        if n == 0 {
            return;
        }
        let cols = 3usize;
        let col_w = pw / cols as f32;
        let row_h = (ph / ((n as f32 / cols as f32).ceil().max(1.0))).max(24.0);

        let mut node_rects = Vec::with_capacity(n);
        for i in 0..n {
            let col = i % cols;
            let row = i / cols;
            let x = px + col as f32 * col_w + col_w * 0.15;
            let y = py + row as f32 * row_h + row_h * 0.15;
            let rw = col_w * 0.70;
            let rh = row_h * 0.70;
            node_rects.push(Rect::new(x, y, rw.max(6.0), rh.max(6.0)));
        }

        let stroke = Stroke::new(1.0);
        for &(a, b, wv) in self.links.iter().take(self.style.max_links) {
            if a >= n || b >= n {
                continue;
            }
            let ra = node_rects[a];
            let rb = node_rects[b];
            let p0 = Point::new(ra.x() + ra.width(), ra.y() + ra.height() * 0.5);
            let p1 = Point::new(rb.x(), rb.y() + rb.height() * 0.5);
            let mx = (p0.x + p1.x) * 0.5;
            let path = Path::new()
                .move_to(p0.x, p0.y)
                .cubic_to(mx, p0.y, mx, p1.y, p1.x, p1.y);
            let alpha = (wv / 10.0).clamp(0.10, 0.55);
            ctx.stroke_path(
                &path,
                &Stroke::new(2.0),
                Brush::Solid(Color::rgba(0.85, 0.92, 1.0, alpha)),
            );
        }

        for (i, r) in node_rects.iter().enumerate() {
            ctx.fill_rect(
                *r,
                8.0.into(),
                Brush::Solid(Color::rgba(0.35, 0.65, 1.0, 0.35)),
            );
            ctx.stroke_rect(
                *r,
                8.0.into(),
                &stroke,
                Brush::Solid(self.style.border_color()),
            );
            if r.width() >= 60.0 {
                let style = TextStyle::new(11.0).with_color(self.style.text);
                ctx.draw_text(
                    &self.labels[i],
                    Point::new(r.x() + 6.0, r.y() + 6.0),
                    &style,
                );
            }
        }
    }

    fn render_chord(&self, ctx: &mut dyn DrawContext, px: f32, py: f32, pw: f32, ph: f32) {
        let n = self.labels.len().min(self.style.max_nodes).max(1);
        let cx = px + pw * 0.5;
        let cy = py + ph * 0.5;
        let r = (pw.min(ph) * 0.42).max(10.0);

        let node_pts = Self::circle_layout(n, r);
        let stroke = Stroke::new(1.0);
        for (i, p) in node_pts.iter().enumerate() {
            // outer ring marker
            ctx.fill_circle(
                Point::new(cx + p.x, cy + p.y),
                self.style.node_radius.max(2.0),
                Brush::Solid(self.leaf_color(i)),
            );
        }

        for &(a, b, wv) in self.links.iter().take(self.style.max_links) {
            if a >= n || b >= n {
                continue;
            }
            let pa = node_pts[a];
            let pb = node_pts[b];
            let p0 = Point::new(cx + pa.x, cy + pa.y);
            let p1 = Point::new(cx + pb.x, cy + pb.y);
            let mx = cx;
            let my = cy;
            let path = Path::new().move_to(p0.x, p0.y).quad_to(mx, my, p1.x, p1.y);
            let alpha = (wv / 10.0).clamp(0.08, 0.35);
            ctx.stroke_path(
                &path,
                &Stroke::new(1.5),
                Brush::Solid(Color::rgba(0.85, 0.92, 1.0, alpha)),
            );
        }

        // outline ring
        ctx.stroke_circle(
            Point::new(cx, cy),
            r,
            &stroke,
            Brush::Solid(Color::rgba(1.0, 1.0, 1.0, 0.12)),
        );
    }

    fn leaf_color(&self, i: usize) -> Color {
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
}

impl NetworkChartStyle {
    fn border_color(&self) -> Color {
        Color::rgba(1.0, 1.0, 1.0, 0.12)
    }
}

#[derive(Clone)]
pub struct NetworkChartHandle(pub Arc<Mutex<NetworkChartModel>>);

impl NetworkChartHandle {
    pub fn new(model: NetworkChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn network_chart(handle: NetworkChartHandle) -> impl ElementBuilder {
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
                    let (px, py, _pw, _ph) = m.plot_rect(bounds.width, bounds.height);
                    if let Some(i) = m.hover_node {
                        let style = TextStyle::new(12.0).with_color(m.style.text);
                        let lbl = m.labels.get(i).map(|s| s.as_str()).unwrap_or("?");
                        ctx.draw_text(
                            &format!("node={i} ({lbl})"),
                            Point::new(px + 6.0, py + 24.0),
                            &style,
                        );
                    }
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
    use blinc_core::{RecordingContext, Size};

    #[test]
    fn sankey_hover_uses_sankey_layout_positions() {
        let mut model = NetworkChartModel::new_sankey(
            vec!["A".into(), "B".into(), "C".into()],
            vec![(0, 1, 1.0), (1, 2, 1.0)],
        )
        .unwrap();

        let (px, py, pw, ph) = model.plot_rect(300.0, 200.0);
        let cols = 3usize;
        let col_w = pw / cols as f32;
        let row_h = (ph / ((3.0f32 / cols as f32).ceil().max(1.0))).max(24.0);

        let x = px + col_w * 0.15 + col_w * 0.70 * 0.5;
        let y = py + row_h * 0.15 + row_h * 0.70 * 0.5;
        model.on_mouse_move(x, y, 300.0, 200.0);

        assert_eq!(model.hover_node, Some(0));
    }

    #[test]
    fn graph_render_does_not_panic_when_max_nodes_exceeds_layout_size() {
        let nodes: Vec<String> = (0..600).map(|i| format!("n{i}")).collect();
        let mut model = NetworkChartModel::new_graph(nodes, vec![(550, 551)]).unwrap();
        model.style.max_nodes = 600;
        model.style.max_links = 8;

        let mut ctx = RecordingContext::new(Size::new(360.0, 240.0));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            model.render_plot(&mut ctx, 360.0, 240.0);
        }));

        assert!(result.is_ok());
    }
}
