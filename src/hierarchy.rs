use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, DrawContext, Path, Point, Rect, Stroke, TextStyle};
use blinc_layout::canvas::canvas;
use blinc_layout::stack::stack;
use blinc_layout::ElementBuilder;

use crate::common::{draw_grid, fill_bg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HierarchyMode {
    Treemap,
    Icicle,
    Sunburst,
    Packing,
}

impl Default for HierarchyMode {
    fn default() -> Self {
        Self::Treemap
    }
}

#[derive(Clone, Debug)]
pub struct HierarchyChartStyle {
    pub bg: Color,
    pub grid: Color,
    pub text: Color,
    pub border: Color,

    pub mode: HierarchyMode,

    /// Hard cap for leaf count drawn.
    pub max_leaves: usize,
}

impl Default for HierarchyChartStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgba(0.08, 0.09, 0.11, 1.0),
            grid: Color::rgba(1.0, 1.0, 1.0, 0.08),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            border: Color::rgba(1.0, 1.0, 1.0, 0.10),
            mode: HierarchyMode::Treemap,
            max_leaves: 2_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HierarchyNode {
    pub label: String,
    pub value: f32,
    pub children: Vec<HierarchyNode>,
}

impl HierarchyNode {
    pub fn leaf(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value,
            children: Vec::new(),
        }
    }

    pub fn node(label: impl Into<String>, children: Vec<HierarchyNode>) -> Self {
        Self {
            label: label.into(),
            value: 0.0,
            children,
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn weight(&self) -> f32 {
        if self.is_leaf() {
            self.value
        } else {
            self.children.iter().map(|c| c.weight()).sum()
        }
    }

    fn validate_finite(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.label.is_empty(), "HierarchyNode label must be non-empty");
        if self.is_leaf() {
            anyhow::ensure!(self.value.is_finite(), "Hierarchy leaf value must be finite");
        }
        for c in &self.children {
            c.validate_finite()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct LeafRect {
    rect: Rect,
    label: String,
    value: f32,
    depth: usize,
}

pub struct HierarchyChartModel {
    pub root: HierarchyNode,
    pub style: HierarchyChartStyle,

    hover_leaf: Option<LeafRect>,
    last_layout_key: Option<(u32, u32, HierarchyMode)>,
    leaves_px: Vec<LeafRect>,
}

impl HierarchyChartModel {
    pub fn new(root: HierarchyNode) -> anyhow::Result<Self> {
        root.validate_finite()?;
        anyhow::ensure!(
            root.weight().is_finite() && root.weight() > 0.0,
            "HierarchyChartModel requires positive finite total weight"
        );

        Ok(Self {
            root,
            style: HierarchyChartStyle::default(),
            hover_leaf: None,
            last_layout_key: None,
            leaves_px: Vec::new(),
        })
    }

    fn plot_rect(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        // Reuse the same padding feel as ChartView (without introducing a domain).
        let left = 16.0;
        let top = 16.0;
        let right = 16.0;
        let bottom = 16.0;
        let pw = (w - left - right).max(0.0);
        let ph = (h - top - bottom).max(0.0);
        (left, top, pw, ph)
    }

    fn leaf_color(&self, depth: usize, i: usize) -> Color {
        // Deterministic palette with depth tint.
        let hues = [
            (0.35, 0.65, 1.0),
            (0.95, 0.55, 0.35),
            (0.40, 0.85, 0.55),
            (0.90, 0.75, 0.25),
            (0.75, 0.55, 0.95),
            (0.25, 0.80, 0.85),
        ];
        let (mut r, mut g, mut b) = hues[i % hues.len()];
        let tint = 1.0 - (depth as f32 * 0.08).clamp(0.0, 0.35);
        r *= tint;
        g *= tint;
        b *= tint;
        Color::rgba(r, g, b, 0.75)
    }

    fn ensure_layout(&mut self, w: f32, h: f32) {
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.leaves_px.clear();
            self.last_layout_key = None;
            return;
        }
        let key = (pw.to_bits(), ph.to_bits(), self.style.mode);
        if self.last_layout_key == Some(key) {
            return;
        }
        self.leaves_px.clear();

        // Avoid borrowing `self.root` immutably while mutably borrowing `self`
        // through the recursive layout functions.
        let root = self.root.clone();

        match self.style.mode {
            HierarchyMode::Treemap => {
                self.layout_treemap(&root, Rect::new(px, py, pw, ph), 0, true);
            }
            HierarchyMode::Icicle => {
                let max_depth = self.max_depth(&root);
                let level_h = (ph / (max_depth.max(1) as f32 + 1.0)).max(1.0);
                self.layout_icicle(
                    &root,
                    Rect::new(px, py, pw, level_h),
                    0,
                    max_depth,
                );
            }
            HierarchyMode::Sunburst => {
                let max_depth = self.max_depth(&root);
                let r = (pw.min(ph) * 0.48).max(10.0);
                let cx = px + pw * 0.5;
                let cy = py + ph * 0.5;
                self.layout_sunburst(
                    &root,
                    0.0,
                    std::f32::consts::TAU,
                    0,
                    max_depth,
                    cx,
                    cy,
                    r,
                );
            }
            HierarchyMode::Packing => {
                let cx = px + pw * 0.5;
                let cy = py + ph * 0.5;
                let r = (pw.min(ph) * 0.48).max(10.0);
                self.layout_packing(&root, cx, cy, r);
            }
        }

        // Hard cap to avoid overshooting GPU primitive budgets.
        if self.leaves_px.len() > self.style.max_leaves {
            self.leaves_px.truncate(self.style.max_leaves);
        }

        self.last_layout_key = Some(key);
    }

    fn max_depth(&self, n: &HierarchyNode) -> usize {
        if n.children.is_empty() {
            0
        } else {
            1 + n.children.iter().map(|c| self.max_depth(c)).max().unwrap_or(0)
        }
    }

    fn layout_treemap(&mut self, n: &HierarchyNode, rect: Rect, depth: usize, split_x: bool) {
        if n.children.is_empty() {
            self.leaves_px.push(LeafRect {
                rect,
                label: n.label.clone(),
                value: n.value,
                depth,
            });
            return;
        }
        let total: f32 = n.children.iter().map(|c| c.weight().max(0.0)).sum();
        if !(total > 0.0) {
            return;
        }

        let mut cur = 0.0f32;
        for c in &n.children {
            let w = c.weight().max(0.0);
            if w <= 0.0 {
                continue;
            }
            let t0 = cur / total;
            cur += w;
            let t1 = cur / total;

            let child_rect = if split_x {
                let x0 = rect.x() + rect.width() * t0;
                let x1 = rect.x() + rect.width() * t1;
                Rect::new(x0, rect.y(), (x1 - x0).max(0.0), rect.height())
            } else {
                let y0 = rect.y() + rect.height() * t0;
                let y1 = rect.y() + rect.height() * t1;
                Rect::new(rect.x(), y0, rect.width(), (y1 - y0).max(0.0))
            };
            self.layout_treemap(c, child_rect, depth + 1, !split_x);
        }
    }

    fn layout_icicle(
        &mut self,
        n: &HierarchyNode,
        rect: Rect,
        depth: usize,
        max_depth: usize,
    ) {
        if depth >= max_depth || n.children.is_empty() {
            self.leaves_px.push(LeafRect {
                rect,
                label: n.label.clone(),
                value: n.weight(),
                depth,
            });
            return;
        }

        let level_h = rect.height().max(1.0);
        let next_y = rect.y() + level_h;
        let total: f32 = n.children.iter().map(|c| c.weight().max(0.0)).sum();
        if !(total > 0.0) {
            return;
        }

        let mut cur = 0.0f32;
        for c in &n.children {
            let w = c.weight().max(0.0);
            if w <= 0.0 {
                continue;
            }
            let t0 = cur / total;
            cur += w;
            let t1 = cur / total;

            let x0 = rect.x() + rect.width() * t0;
            let x1 = rect.x() + rect.width() * t1;
            let child_rect = Rect::new(x0, next_y, (x1 - x0).max(0.0), level_h);
            self.layout_icicle(c, child_rect, depth + 1, max_depth);
        }
    }

    fn layout_sunburst(
        &mut self,
        n: &HierarchyNode,
        a0: f32,
        a1: f32,
        depth: usize,
        max_depth: usize,
        cx: f32,
        cy: f32,
        r: f32,
    ) {
        if n.children.is_empty() || depth >= max_depth {
            let dr = r / (max_depth.max(1) as f32 + 1.0);
            let r0 = depth as f32 * dr;
            let r1 = (depth as f32 + 1.0) * dr;
            // Store as a rect placeholder; render uses (x,y,w,h) to encode the sector.
            self.leaves_px.push(LeafRect {
                rect: Rect::new(a0, a1, r0, r1),
                label: n.label.clone(),
                value: n.weight(),
                depth,
            });
            return;
        }

        let total: f32 = n.children.iter().map(|c| c.weight().max(0.0)).sum();
        if !(total > 0.0) {
            return;
        }
        let mut cur = a0;
        for c in &n.children {
            let w = c.weight().max(0.0);
            if w <= 0.0 {
                continue;
            }
            let span = (a1 - a0) * (w / total);
            let next = cur + span;
            self.layout_sunburst(c, cur, next, depth + 1, max_depth, cx, cy, r);
            cur = next;
        }
    }

    fn layout_packing(&mut self, n: &HierarchyNode, cx: f32, cy: f32, r: f32) {
        // Naive packing: place leaf circles along a spiral, with radius ~ sqrt(weight).
        let mut leaves = Vec::new();
        self.collect_leaves(n, 0, &mut leaves);
        if leaves.is_empty() {
            return;
        }
        leaves.sort_by(|a, b| b.2.total_cmp(&a.2));

        let total: f32 = leaves.iter().map(|l| l.2.max(0.0)).sum();
        if !(total > 0.0) {
            return;
        }

        let mut placed: Vec<(f32, f32, f32, String, f32, usize)> = Vec::new(); // (x,y,rad,label,val,depth)
        for (i, (label, depth, w)) in leaves.into_iter().enumerate() {
            let rr = ((w / total).max(0.0)).sqrt() * r * 0.75;
            let rr = rr.clamp(4.0, r * 0.35);
            if i == 0 {
                placed.push((0.0, 0.0, rr, label, w, depth));
                continue;
            }

            let mut ang = 0.0f32;
            let mut rad = 0.0f32;
            let mut best = None;
            for _ in 0..2_000 {
                ang += 0.35;
                rad += 0.18;
                let x = rad * ang.cos();
                let y = rad * ang.sin();
                if x * x + y * y > (r - rr).powi(2) {
                    continue;
                }
                let mut ok = true;
                for (px, py, pr, ..) in &placed {
                    let dx = x - *px;
                    let dy = y - *py;
                    if dx * dx + dy * dy < (*pr + rr).powi(2) * 1.02 {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    best = Some((x, y));
                    break;
                }
            }
            let (x, y) = best.unwrap_or((0.0, 0.0));
            placed.push((x, y, rr, label, w, depth));
        }

        for (x, y, rr, label, v, depth) in placed {
            self.leaves_px.push(LeafRect {
                rect: Rect::new(cx + x, cy + y, rr, rr),
                label,
                value: v,
                depth,
            });
        }
    }

    fn collect_leaves(&self, n: &HierarchyNode, depth: usize, out: &mut Vec<(String, usize, f32)>) {
        if n.children.is_empty() {
            out.push((n.label.clone(), depth, n.value.max(0.0)));
            return;
        }
        for c in &n.children {
            self.collect_leaves(c, depth + 1, out);
        }
    }

    pub fn on_mouse_move(&mut self, local_x: f32, local_y: f32, w: f32, h: f32) {
        self.ensure_layout(w, h);

        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            self.hover_leaf = None;
            return;
        }
        if local_x < px || local_x > px + pw || local_y < py || local_y > py + ph {
            self.hover_leaf = None;
            return;
        }

        match self.style.mode {
            HierarchyMode::Sunburst => {
                // Hover detection for sunburst is approximate: find the first sector
                // whose annulus wedge contains the point.
                let cx = px + pw * 0.5;
                let cy = py + ph * 0.5;
                let dx = local_x - cx;
                let dy = local_y - cy;
                let rr = (dx * dx + dy * dy).sqrt();
                let mut a = dy.atan2(dx);
                if a < 0.0 {
                    a += std::f32::consts::TAU;
                }
                self.hover_leaf = None;
                for leaf in &self.leaves_px {
                    let a0 = leaf.rect.x();
                    let a1 = leaf.rect.y();
                    let r0 = leaf.rect.width();
                    let r1 = leaf.rect.height();
                    if rr >= r0 && rr <= r1 && a >= a0 && a <= a1 {
                        self.hover_leaf = Some(leaf.clone());
                        break;
                    }
                }
            }
            HierarchyMode::Packing => {
                self.hover_leaf = None;
                for leaf in &self.leaves_px {
                    let cx = leaf.rect.x();
                    let cy = leaf.rect.y();
                    let r = leaf.rect.width();
                    let dx = local_x - cx;
                    let dy = local_y - cy;
                    if dx * dx + dy * dy <= r * r {
                        self.hover_leaf = Some(leaf.clone());
                        break;
                    }
                }
            }
            _ => {
                self.hover_leaf = None;
                for leaf in &self.leaves_px {
                    let r = leaf.rect;
                    if local_x >= r.x()
                        && local_x <= r.x() + r.width()
                        && local_y >= r.y()
                        && local_y <= r.y() + r.height()
                    {
                        self.hover_leaf = Some(leaf.clone());
                        break;
                    }
                }
            }
        }
    }

    pub fn render_plot(&mut self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        fill_bg(ctx, w, h, self.style.bg);
        let (px, py, pw, ph) = self.plot_rect(w, h);
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        draw_grid(ctx, px, py, pw, ph, self.style.grid, 4);

        self.ensure_layout(w, h);

        match self.style.mode {
            HierarchyMode::Sunburst => {
                let max_depth = self.max_depth(&self.root).max(1);
                let r = (pw.min(ph) * 0.48).max(10.0);
                let cx = px + pw * 0.5;
                let cy = py + ph * 0.5;
                let dr = r / (max_depth as f32 + 1.0);

                for (i, leaf) in self.leaves_px.iter().enumerate() {
                    // Stored as (a0,a1,r0,r1).
                    let a0 = leaf.rect.x();
                    let a1 = leaf.rect.y();
                    let r0 = leaf.rect.width();
                    let r1 = leaf.rect.height();
                    let r0 = r0.min(r);
                    let r1 = r1.min(r);
                    if !(a1 > a0) || !(r1 > r0) {
                        continue;
                    }

                    let segs = (((a1 - a0) * (r1 / dr)).abs() * 6.0) as usize;
                    let segs = segs.clamp(6, 48);

                    let mut pts = Vec::with_capacity(segs * 2 + 2);
                    for s in 0..=segs {
                        let t = s as f32 / segs as f32;
                        let a = a0 + (a1 - a0) * t;
                        pts.push(Point::new(cx + r1 * a.cos(), cy + r1 * a.sin()));
                    }
                    for s in (0..=segs).rev() {
                        let t = s as f32 / segs as f32;
                        let a = a0 + (a1 - a0) * t;
                        pts.push(Point::new(cx + r0 * a.cos(), cy + r0 * a.sin()));
                    }
                    let mut path = Path::new().move_to(pts[0].x, pts[0].y);
                    for p in &pts[1..] {
                        path = path.line_to(p.x, p.y);
                    }
                    path = path.close();
                    ctx.fill_path(
                        &path,
                        Brush::Solid(self.leaf_color(leaf.depth, i)),
                    );
                }

                // Outline rings for readability.
                let stroke = Stroke::new(1.0);
                for d in 1..=max_depth + 1 {
                    ctx.stroke_circle(
                        Point::new(cx, cy),
                        dr * d as f32,
                        &stroke,
                        Brush::Solid(self.style.border),
                    );
                }
            }
            HierarchyMode::Packing => {
                let stroke = Stroke::new(1.0);
                for (i, leaf) in self.leaves_px.iter().enumerate() {
                    let cx = leaf.rect.x();
                    let cy = leaf.rect.y();
                    let r = leaf.rect.width();
                    ctx.fill_circle(
                        Point::new(cx, cy),
                        r,
                        Brush::Solid(self.leaf_color(leaf.depth, i)),
                    );
                    ctx.stroke_circle(
                        Point::new(cx, cy),
                        r,
                        &stroke,
                        Brush::Solid(self.style.border),
                    );
                }
            }
            _ => {
                let stroke = Stroke::new(1.0);
                for (i, leaf) in self.leaves_px.iter().enumerate() {
                    if leaf.rect.width() < 1.0 || leaf.rect.height() < 1.0 {
                        continue;
                    }
                    ctx.fill_rect(
                        leaf.rect,
                        6.0.into(),
                        Brush::Solid(self.leaf_color(leaf.depth, i)),
                    );
                    ctx.stroke_rect(
                        leaf.rect,
                        6.0.into(),
                        &stroke,
                        Brush::Solid(self.style.border),
                    );

                    // Label only if there is room.
                    if leaf.rect.width() >= 70.0 && leaf.rect.height() >= 18.0 {
                        let style = TextStyle::new(11.0).with_color(self.style.text);
                        ctx.draw_text(
                            &leaf.label,
                            Point::new(leaf.rect.x() + 6.0, leaf.rect.y() + 6.0),
                            &style,
                        );
                    }
                }
            }
        }

        let style = TextStyle::new(12.0).with_color(self.style.text);
        ctx.draw_text(
            match self.style.mode {
                HierarchyMode::Treemap => "treemap",
                HierarchyMode::Icicle => "icicle",
                HierarchyMode::Sunburst => "sunburst",
                HierarchyMode::Packing => "packing",
            },
            Point::new(px + 6.0, py + 6.0),
            &style,
        );
    }

    pub fn render_overlay(&self, ctx: &mut dyn DrawContext, w: f32, h: f32) {
        let (px, py, _pw, _ph) = self.plot_rect(w, h);
        if let Some(l) = &self.hover_leaf {
            let style = TextStyle::new(12.0).with_color(self.style.text);
            ctx.draw_text(
                &format!("{}  (v={:.2})", l.label, l.value),
                Point::new(px + 6.0, py + 24.0),
                &style,
            );
        }
    }
}

#[derive(Clone)]
pub struct HierarchyChartHandle(pub Arc<Mutex<HierarchyChartModel>>);

impl HierarchyChartHandle {
    pub fn new(model: HierarchyChartModel) -> Self {
        Self(Arc::new(Mutex::new(model)))
    }
}

pub fn hierarchy_chart(handle: HierarchyChartHandle) -> impl ElementBuilder {
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
                    m.render_overlay(ctx, bounds.width, bounds.height);
                }
            })
            .w_full()
            .h_full()
            .foreground(),
        )
}
