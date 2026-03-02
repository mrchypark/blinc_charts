use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Path, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::common::{draw_grid, fill_bg};
use crate::palette;
use crate::spatial_index::SpatialIndex;
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

#[derive(Clone, Debug, Default)]
struct SankeyLayout {
    node_rects: Vec<Rect>,
    link_paths: Vec<(Path, f32, f32)>, // (path, thickness, alpha)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GraphHoverCacheKey {
    domain_x_min: u32,
    domain_x_max: u32,
    domain_y_min: u32,
    domain_y_max: u32,
    plot_x: u32,
    plot_y: u32,
    plot_w: u32,
    plot_h: u32,
    max_nodes: usize,
}

impl GraphHoverCacheKey {
    fn new(view: &ChartView, px: f32, py: f32, pw: f32, ph: f32, max_nodes: usize) -> Self {
        Self {
            domain_x_min: view.domain.x.min.to_bits(),
            domain_x_max: view.domain.x.max.to_bits(),
            domain_y_min: view.domain.y.min.to_bits(),
            domain_y_max: view.domain.y.max.to_bits(),
            plot_x: px.to_bits(),
            plot_y: py.to_bits(),
            plot_w: pw.to_bits(),
            plot_h: ph.to_bits(),
            max_nodes,
        }
    }
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
    graph_hover_points_px: Vec<Point>,
    graph_hover_index: Option<SpatialIndex>,
    graph_hover_cache_key: Option<GraphHoverCacheKey>,
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
            graph_hover_points_px: Vec::new(),
            graph_hover_index: None,
            graph_hover_cache_key: None,
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
            graph_hover_points_px: Vec::new(),
            graph_hover_index: None,
            graph_hover_cache_key: None,
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

    fn ensure_graph_hover_cache(&mut self, px: f32, py: f32, pw: f32, ph: f32, max_n: usize) {
        let key = GraphHoverCacheKey::new(&self.view, px, py, pw, ph, max_n);
        if self.graph_hover_cache_key == Some(key) {
            return;
        }

        self.graph_hover_points_px.clear();
        self.graph_hover_points_px.reserve(max_n);
        for i in 0..max_n {
            self.graph_hover_points_px.push(self.view.data_to_px(
                self.layout.node_pos[i],
                px,
                py,
                pw,
                ph,
            ));
        }

        self.graph_hover_index = if max_n >= 48 {
            Some(SpatialIndex::build(&self.graph_hover_points_px, 16, 16))
        } else {
            None
        };
        self.graph_hover_cache_key = Some(key);
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
                let max_n = self
                    .labels
                    .len()
                    .min(self.style.max_nodes)
                    .min(self.layout.node_pos.len());
                if max_n == 0 {
                    self.hover_node = None;
                    return;
                }
                self.ensure_graph_hover_cache(px, py, pw, ph, max_n);

                if let Some(index) = self.graph_hover_index.as_ref() {
                    self.hover_node = index.nearest(local_x, local_y, hit_r).map(|(i, _)| i);
                } else {
                    let mut best = None::<(usize, f32)>;
                    for (i, sp) in self.graph_hover_points_px.iter().enumerate() {
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
            NetworkMode::Sankey => {
                let layout = self.build_sankey_layout(px, py, pw, ph);
                if layout.node_rects.is_empty() {
                    self.hover_node = None;
                    return;
                }

                let cursor = Point::new(local_x, local_y);
                if let Some((i, _)) = layout
                    .node_rects
                    .iter()
                    .enumerate()
                    .find(|(_, r)| r.contains(cursor))
                {
                    self.hover_node = Some(i);
                    return;
                }

                let mut best = None::<(usize, f32)>;
                for (i, r) in layout.node_rects.iter().enumerate() {
                    let c = r.center();
                    let dx = c.x - local_x;
                    let dy = c.y - local_y;
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
        let layout = self.build_sankey_layout(px, py, pw, ph);
        if layout.node_rects.is_empty() {
            return;
        }

        let stroke = Stroke::new(1.0);
        for (path, thickness, alpha) in layout.link_paths {
            ctx.stroke_path(
                &path,
                &Stroke::new(thickness),
                Brush::Solid(Color::rgba(0.85, 0.92, 1.0, alpha)),
            );
        }

        for (i, r) in layout.node_rects.iter().enumerate() {
            let c = self.leaf_color(i);
            ctx.fill_rect(
                *r,
                8.0.into(),
                Brush::Solid(Color::rgba(c.r, c.g, c.b, 0.35)),
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

    fn build_sankey_layout(&self, px: f32, py: f32, pw: f32, ph: f32) -> SankeyLayout {
        let n = self.labels.len().min(self.style.max_nodes);
        if n == 0 || pw <= 0.0 || ph <= 0.0 {
            return SankeyLayout::default();
        }

        let mut links = Vec::new();
        let mut in_flow = vec![0.0f32; n];
        let mut out_flow = vec![0.0f32; n];
        for &(a, b, w) in self.links.iter().take(self.style.max_links) {
            if a >= n || b >= n || !w.is_finite() || w <= 0.0 {
                continue;
            }
            links.push((a, b, w));
            out_flow[a] += w;
            in_flow[b] += w;
        }

        let mut layer = vec![0usize; n];
        let mut indegree = vec![0usize; n];
        let mut adj = vec![Vec::<usize>::new(); n];
        for &(a, b, _) in &links {
            indegree[b] += 1;
            adj[a].push(b);
        }

        let mut q = VecDeque::new();
        for (i, &deg) in indegree.iter().enumerate() {
            if deg == 0 {
                q.push_back(i);
            }
        }
        let mut visited = 0usize;
        while let Some(u) = q.pop_front() {
            visited += 1;
            for &v in &adj[u] {
                layer[v] = layer[v].max(layer[u] + 1);
                indegree[v] -= 1;
                if indegree[v] == 0 {
                    q.push_back(v);
                }
            }
        }
        if visited < n {
            let fallback_layers = 3usize.min(n.max(1));
            for i in 0..n {
                if indegree[i] > 0 {
                    layer[i] = i % fallback_layers;
                }
            }
        }

        let mut max_layer = *layer.iter().max().unwrap_or(&0);
        if max_layer == 0 && n > 1 {
            for i in 0..n {
                if in_flow[i] == 0.0 && out_flow[i] > 0.0 {
                    layer[i] = 0;
                } else if in_flow[i] > 0.0 && out_flow[i] == 0.0 {
                    layer[i] = 2;
                } else {
                    layer[i] = 1;
                }
            }
            max_layer = *layer.iter().max().unwrap_or(&0);
        }
        for i in 0..n {
            if in_flow[i] > 0.0 && out_flow[i] == 0.0 {
                layer[i] = max_layer.max(1);
            }
        }
        max_layer = *layer.iter().max().unwrap_or(&0);

        let mut layer_nodes = vec![Vec::<usize>::new(); max_layer + 1];
        for i in 0..n {
            layer_nodes[layer[i]].push(i);
        }
        // Initialize with stable order by flow, then refine with barycentric sweeps.
        for nodes in &mut layer_nodes {
            nodes.sort_by(|a, b| {
                let fa = in_flow[*a] + out_flow[*a];
                let fb = in_flow[*b] + out_flow[*b];
                fb.partial_cmp(&fa)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.cmp(b))
            });
        }
        let mut layer_pos = vec![0usize; n];
        for nodes in &layer_nodes {
            for (i, &node) in nodes.iter().enumerate() {
                layer_pos[node] = i;
            }
        }

        for _ in 0..4 {
            // Left -> right sweep using incoming neighbors.
            for li in 1..=max_layer {
                let mut scored = Vec::with_capacity(layer_nodes[li].len());
                for &node in &layer_nodes[li] {
                    let mut sum = 0.0f32;
                    let mut cnt = 0usize;
                    for &(a, b, _w) in &links {
                        if b == node && layer[a] + 1 == li {
                            sum += layer_pos[a] as f32;
                            cnt += 1;
                        }
                    }
                    let bary = if cnt > 0 {
                        sum / cnt as f32
                    } else {
                        layer_pos[node] as f32
                    };
                    scored.push((node, bary, layer_pos[node]));
                }
                scored.sort_by(|a, b| {
                    a.1.partial_cmp(&b.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.2.cmp(&b.2))
                });
                layer_nodes[li] = scored.into_iter().map(|s| s.0).collect();
                for (i, &node) in layer_nodes[li].iter().enumerate() {
                    layer_pos[node] = i;
                }
            }

            // Right -> left sweep using outgoing neighbors.
            if max_layer > 0 {
                for li in (0..max_layer).rev() {
                    let mut scored = Vec::with_capacity(layer_nodes[li].len());
                    for &node in &layer_nodes[li] {
                        let mut sum = 0.0f32;
                        let mut cnt = 0usize;
                        for &(a, b, _w) in &links {
                            if a == node && layer[b] == li + 1 {
                                sum += layer_pos[b] as f32;
                                cnt += 1;
                            }
                        }
                        let bary = if cnt > 0 {
                            sum / cnt as f32
                        } else {
                            layer_pos[node] as f32
                        };
                        scored.push((node, bary, layer_pos[node]));
                    }
                    scored.sort_by(|a, b| {
                        a.1.partial_cmp(&b.1)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| a.2.cmp(&b.2))
                    });
                    layer_nodes[li] = scored.into_iter().map(|s| s.0).collect();
                    for (i, &node) in layer_nodes[li].iter().enumerate() {
                        layer_pos[node] = i;
                    }
                }
            }
        }

        let layer_count = max_layer + 1;
        let node_w = (pw / (layer_count as f32 * 8.0)).clamp(10.0, 24.0);
        let x_step = if layer_count > 1 {
            (pw - node_w).max(0.0) / (layer_count - 1) as f32
        } else {
            0.0
        };

        let mut node_rects = vec![Rect::ZERO; n];
        for (li, nodes) in layer_nodes.iter().enumerate() {
            let m = nodes.len();
            if m == 0 {
                continue;
            }
            let gap = (ph * 0.03).clamp(6.0, 18.0);
            let total_gap = gap * (m as f32 + 1.0);
            let usable_h = (ph - total_gap).max(8.0 * m as f32);
            let min_h = 8.0f32;
            let base_h = min_h * m as f32;
            let extra_h = (usable_h - base_h).max(0.0);

            let layer_flow: f32 = nodes
                .iter()
                .map(|&idx| in_flow[idx].max(out_flow[idx]).max(1.0))
                .sum();

            let x = px + li as f32 * x_step;
            let mut y = py + gap;
            for &idx in nodes {
                let f = in_flow[idx].max(out_flow[idx]).max(1.0);
                let extra = if layer_flow > 0.0 {
                    extra_h * (f / layer_flow)
                } else {
                    extra_h / m as f32
                };
                let h = min_h + extra;
                node_rects[idx] = Rect::new(x, y, node_w, h.max(min_h));
                y += h + gap;
            }
        }

        links.sort_by_key(|(a, b, _)| (layer[*a], *a, *b));
        let mut src_offsets = vec![0.0f32; n];
        let mut dst_offsets = vec![0.0f32; n];
        let mut link_paths = Vec::with_capacity(links.len());

        for &(a, b, w) in &links {
            let ra = node_rects[a];
            let rb = node_rects[b];
            if ra.width() <= 0.0 || ra.height() <= 0.0 || rb.width() <= 0.0 || rb.height() <= 0.0 {
                continue;
            }

            let src_scale = ra.height() / out_flow[a].max(1e-6);
            let dst_scale = rb.height() / in_flow[b].max(1e-6);
            let mut thickness = (w * src_scale.min(dst_scale)).clamp(1.0, 24.0);
            let rem_src = (ra.height() - src_offsets[a]).max(1.0);
            let rem_dst = (rb.height() - dst_offsets[b]).max(1.0);
            thickness = thickness.min(rem_src).min(rem_dst).max(1.0);

            let y0 = ra.y() + src_offsets[a] + thickness * 0.5;
            let y1 = rb.y() + dst_offsets[b] + thickness * 0.5;
            src_offsets[a] += thickness;
            dst_offsets[b] += thickness;

            let x0 = ra.x() + ra.width();
            let x1 = rb.x();
            let dx = (x1 - x0).abs() * 0.5;
            let c0x = if x1 >= x0 { x0 + dx } else { x0 - dx };
            let c1x = if x1 >= x0 { x1 - dx } else { x1 + dx };
            let path = Path::new()
                .move_to(x0, y0)
                .cubic_to(c0x, y0, c1x, y1, x1, y1);

            let alpha = (0.10 + thickness / 26.0).clamp(0.10, 0.62);
            link_paths.push((path, thickness, alpha));
        }

        SankeyLayout {
            node_rects,
            link_paths,
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
        palette::qualitative(i, 0.85)
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
        let layout = model.build_sankey_layout(px, py, pw, ph);
        let c = layout.node_rects[0].center();
        let x = c.x;
        let y = c.y;
        model.on_mouse_move(x, y, 300.0, 200.0);

        assert_eq!(model.hover_node, Some(0));
    }

    #[test]
    fn sankey_layout_orders_sources_before_sinks() {
        let model = NetworkChartModel::new_sankey(
            vec!["src".into(), "mid".into(), "sink".into()],
            vec![(0, 1, 3.0), (1, 2, 2.0)],
        )
        .unwrap();

        let (px, py, pw, ph) = model.plot_rect(320.0, 220.0);
        let layout = model.build_sankey_layout(px, py, pw, ph);
        assert!(layout.node_rects[0].x() < layout.node_rects[1].x());
        assert!(layout.node_rects[1].x() < layout.node_rects[2].x());
        assert!(!layout.link_paths.is_empty());
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

    #[test]
    fn graph_hover_remains_correct_across_repeated_moves_and_zoom() {
        let nodes: Vec<String> = (0..128).map(|i| format!("n{i}")).collect();
        let mut model = NetworkChartModel::new_graph(nodes, vec![]).unwrap();
        model.style.max_nodes = 128;

        let (px, py, pw, ph) = model.plot_rect(360.0, 240.0);
        let p0 = model
            .view
            .data_to_px(model.layout.node_pos[0], px, py, pw, ph);
        model.on_mouse_move(p0.x, p0.y, 360.0, 240.0);
        assert_eq!(model.hover_node, Some(0));

        model.on_mouse_move(p0.x, p0.y, 360.0, 240.0);
        assert_eq!(model.hover_node, Some(0));

        model.on_scroll(-80.0, p0.x, p0.y, 360.0, 240.0);
        let (px2, py2, pw2, ph2) = model.plot_rect(360.0, 240.0);
        let p0_after_zoom = model
            .view
            .data_to_px(model.layout.node_pos[0], px2, py2, pw2, ph2);
        model.on_mouse_move(p0_after_zoom.x, p0_after_zoom.y, 360.0, 240.0);
        assert_eq!(model.hover_node, Some(0));
    }
}
