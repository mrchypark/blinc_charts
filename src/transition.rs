use crate::interpolate::lerp_f32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueTransition {
    start: f32,
    end: f32,
    duration: f32,
    elapsed: f32,
    value: f32,
}

impl ValueTransition {
    pub fn new(start: f32, end: f32, duration_seconds: f32) -> Self {
        let duration = duration_seconds.max(1e-6);
        Self {
            start,
            end,
            duration,
            elapsed: 0.0,
            value: start,
        }
    }

    pub fn step(&mut self, dt_seconds: f32) {
        if self.is_finished() {
            self.value = self.end;
            return;
        }
        self.elapsed = (self.elapsed + dt_seconds.max(0.0)).min(self.duration);
        let t = self.elapsed / self.duration;
        self.value = lerp_f32(self.start, self.end, t);
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_reaches_target() {
        let mut t = ValueTransition::new(0.0, 1.0, 1.0);
        t.step(0.4);
        assert!(t.value() > 0.0 && t.value() < 1.0);
        t.step(0.6);
        assert_eq!(t.value(), 1.0);
        assert!(t.is_finished());
    }
}
