#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearScale {
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
}

impl LinearScale {
    pub fn new(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
        }
    }

    pub fn map(&self, value: f32) -> f32 {
        let d = self.domain_max - self.domain_min;
        if d.abs() < 1e-12 {
            return self.range_min;
        }
        let t = (value - self.domain_min) / d;
        self.range_min + t * (self.range_max - self.range_min)
    }

    pub fn invert(&self, px: f32) -> f32 {
        let r = self.range_max - self.range_min;
        if r.abs() < 1e-12 {
            return self.domain_min;
        }
        let t = (px - self.range_min) / r;
        self.domain_min + t * (self.domain_max - self.domain_min)
    }

    pub fn ticks(&self, count: usize) -> Vec<f32> {
        let n = count.max(2);
        let mut out = Vec::with_capacity(n);
        let span = self.domain_max - self.domain_min;
        for i in 0..n {
            let t = i as f32 / (n - 1) as f32;
            out.push(self.domain_min + span * t);
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandScale {
    count: usize,
    start: f32,
    step: f32,
    band_width: f32,
}

impl BandScale {
    pub fn new(
        count: usize,
        range_min: f32,
        range_max: f32,
        padding_inner: f32,
        padding_outer: f32,
    ) -> Self {
        if count == 0 {
            return Self {
                count: 0,
                start: range_min,
                step: 0.0,
                band_width: 0.0,
            };
        }
        let count_f = count as f32;
        let span = (range_max - range_min).max(0.0);
        let denom = (count_f - padding_inner + 2.0 * padding_outer).max(1e-6);
        let step = span / denom;
        let band_width = step * (1.0 - padding_inner).max(0.0);
        let start = range_min + step * padding_outer;
        Self {
            count,
            start,
            step,
            band_width,
        }
    }

    pub fn band_width(&self) -> f32 {
        self.band_width
    }

    pub fn band_start(&self, idx: usize) -> Option<f32> {
        if idx >= self.count {
            return None;
        }
        Some(self.start + self.step * idx as f32)
    }

    pub fn center(&self, idx: usize) -> Option<f32> {
        self.band_start(idx).map(|x| x + self.band_width * 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_ticks_include_endpoints() {
        let s = LinearScale::new(10.0, 20.0, 0.0, 100.0);
        let t = s.ticks(4);
        assert_eq!(t[0], 10.0);
        assert_eq!(t[3], 20.0);
    }

    #[test]
    fn band_scale_bounds_indices() {
        let b = BandScale::new(3, 0.0, 300.0, 0.1, 0.05);
        assert!(b.band_start(2).is_some());
        assert!(b.band_start(3).is_none());
    }

    #[test]
    fn linear_invert_handles_descending_range() {
        let s = LinearScale::new(0.0, 100.0, 200.0, 100.0);
        assert!((s.invert(150.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn band_scale_with_zero_count_has_no_band_width() {
        let b = BandScale::new(0, 0.0, 100.0, 0.1, 0.05);
        assert_eq!(b.band_width(), 0.0);
        assert!(b.band_start(0).is_none());
    }
}
