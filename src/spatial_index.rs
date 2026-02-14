use blinc_core::Point;

#[derive(Clone, Debug)]
pub struct SpatialIndex {
    points: Vec<Point>,
    cells: Vec<Vec<usize>>,
    cols: usize,
    rows: usize,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}

impl SpatialIndex {
    pub fn build(points: &[Point], cols: usize, rows: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);

        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for p in points {
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }

        if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
            min_x = 0.0;
            max_x = 1.0;
            min_y = 0.0;
            max_y = 1.0;
        }
        if (max_x - min_x).abs() < 1e-6 {
            max_x = min_x + 1.0;
        }
        if (max_y - min_y).abs() < 1e-6 {
            max_y = min_y + 1.0;
        }

        let mut cells = vec![Vec::new(); cols * rows];
        let mut owned_points = Vec::with_capacity(points.len());
        for (i, p) in points.iter().enumerate() {
            owned_points.push(*p);
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            let c = (((p.x - min_x) / (max_x - min_x)) * cols as f32)
                .floor()
                .clamp(0.0, (cols - 1) as f32) as usize;
            let r = (((p.y - min_y) / (max_y - min_y)) * rows as f32)
                .floor()
                .clamp(0.0, (rows - 1) as f32) as usize;
            cells[r * cols + c].push(i);
        }

        Self {
            points: owned_points,
            cells,
            cols,
            rows,
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }

    pub fn nearest(&self, x: f32, y: f32, max_radius: f32) -> Option<(usize, f32)> {
        if self.points.is_empty() {
            return None;
        }
        let max_radius = max_radius.max(0.0);
        let max_r2 = max_radius * max_radius;
        let cell_w = (self.max_x - self.min_x) / self.cols as f32;
        let cell_h = (self.max_y - self.min_y) / self.rows as f32;

        let cx = (((x - self.min_x) / (self.max_x - self.min_x)) * self.cols as f32)
            .floor()
            .clamp(0.0, (self.cols - 1) as f32) as i32;
        let cy = (((y - self.min_y) / (self.max_y - self.min_y)) * self.rows as f32)
            .floor()
            .clamp(0.0, (self.rows - 1) as f32) as i32;

        let rx = (max_radius / cell_w.max(1e-6)).ceil() as i32;
        let ry = (max_radius / cell_h.max(1e-6)).ceil() as i32;

        let mut best: Option<(usize, f32)> = None;
        for row in (cy - ry).max(0)..=(cy + ry).min(self.rows as i32 - 1) {
            for col in (cx - rx).max(0)..=(cx + rx).min(self.cols as i32 - 1) {
                for &idx in &self.cells[row as usize * self.cols + col as usize] {
                    let p = self.points[idx];
                    if !p.x.is_finite() || !p.y.is_finite() {
                        continue;
                    }
                    let dx = p.x - x;
                    let dy = p.y - y;
                    let d2 = dx * dx + dy * dy;
                    if d2 > max_r2 {
                        continue;
                    }
                    if best.map(|(_, b)| d2 < b).unwrap_or(true) {
                        best = Some((idx, d2));
                    }
                }
            }
        }

        best.map(|(i, d2)| (i, d2.sqrt()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_returns_expected_index() {
        let pts = vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)];
        let idx = SpatialIndex::build(&pts, 4, 4)
            .nearest(9.0, 9.0, 3.0)
            .map(|(i, _)| i);
        assert_eq!(idx, Some(1));
    }
}
