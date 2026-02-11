use std::sync::{Arc, Mutex};

use crate::view::Domain1D;

/// Shared chart state for linking interactions across multiple charts/canvases.
///
/// Design goals:
/// - Pure data (no renderer coupling)
/// - Cheap to clone/share (Arc<Mutex<_>>)
/// - Minimal surface to integrate with external tools (e.g. patch_map)
#[derive(Clone, Copy, Debug)]
pub struct ChartLink {
    /// Shared X domain (time axis) for pan/zoom synchronization.
    pub x_domain: Domain1D,

    /// Shared hover x position in domain units (if any).
    pub hover_x: Option<f32>,

    /// Shared X-range selection in domain units (min..max). Inclusive semantics are up to consumers.
    pub selection_x: Option<(f32, f32)>,
}

impl ChartLink {
    pub fn new(x_domain: Domain1D) -> Self {
        let x_domain = if x_domain.is_valid() {
            x_domain
        } else {
            Domain1D::new(0.0, 1.0)
        };
        Self {
            x_domain,
            hover_x: None,
            selection_x: None,
        }
    }

    pub fn set_x_domain(&mut self, x_domain: Domain1D) {
        if x_domain.is_valid() {
            self.x_domain = x_domain;
        }
    }

    pub fn set_hover_x(&mut self, hover_x: Option<f32>) {
        self.hover_x = hover_x.filter(|v| v.is_finite());
    }

    pub fn set_selection_x(&mut self, selection_x: Option<(f32, f32)>) {
        self.selection_x = selection_x.and_then(|(a, b)| {
            if !(a.is_finite() && b.is_finite()) {
                return None;
            }
            Some(if a <= b { (a, b) } else { (b, a) })
        });
    }
}

pub type ChartLinkHandle = Arc<Mutex<ChartLink>>;

pub fn chart_link(x_min: f32, x_max: f32) -> ChartLinkHandle {
    Arc::new(Mutex::new(ChartLink::new(Domain1D::new(x_min, x_max))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_x_domain_rejects_non_finite_or_inverted_domain() {
        let mut link = ChartLink::new(Domain1D::new(0.0, 10.0));

        link.set_x_domain(Domain1D::new(f32::NAN, 10.0));
        assert_eq!(link.x_domain, Domain1D::new(0.0, 10.0));

        link.set_x_domain(Domain1D::new(10.0, 10.0));
        assert_eq!(link.x_domain, Domain1D::new(0.0, 10.0));

        link.set_x_domain(Domain1D::new(5.0, 15.0));
        assert_eq!(link.x_domain, Domain1D::new(5.0, 15.0));
    }

    #[test]
    fn set_hover_x_drops_non_finite_values() {
        let mut link = ChartLink::new(Domain1D::new(0.0, 1.0));

        link.set_hover_x(Some(0.25));
        assert_eq!(link.hover_x, Some(0.25));

        link.set_hover_x(Some(f32::NAN));
        assert_eq!(link.hover_x, None);

        link.set_hover_x(Some(f32::INFINITY));
        assert_eq!(link.hover_x, None);
    }

    #[test]
    fn set_selection_x_sorts_and_rejects_non_finite_values() {
        let mut link = ChartLink::new(Domain1D::new(0.0, 1.0));

        link.set_selection_x(Some((8.0, 2.0)));
        assert_eq!(link.selection_x, Some((2.0, 8.0)));

        link.set_selection_x(Some((f32::NAN, 3.0)));
        assert_eq!(link.selection_x, None);

        link.set_selection_x(Some((2.0, f32::INFINITY)));
        assert_eq!(link.selection_x, None);
    }

    #[test]
    fn chart_link_sanitizes_invalid_initial_domain() {
        let handle = chart_link(f32::NAN, 10.0);
        let link = handle.lock().expect("chart link lock");
        assert_eq!(link.x_domain, Domain1D::new(0.0, 1.0));
    }
}
