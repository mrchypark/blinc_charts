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
        Self {
            x_domain,
            hover_x: None,
            selection_x: None,
        }
    }

    pub fn set_x_domain(&mut self, x_domain: Domain1D) {
        self.x_domain = x_domain;
    }

    pub fn set_hover_x(&mut self, hover_x: Option<f32>) {
        self.hover_x = hover_x;
    }

    pub fn set_selection_x(&mut self, selection_x: Option<(f32, f32)>) {
        self.selection_x = selection_x.map(|(a, b)| if a <= b { (a, b) } else { (b, a) });
    }
}

pub type ChartLinkHandle = Arc<Mutex<ChartLink>>;

pub fn chart_link(x_min: f32, x_max: f32) -> ChartLinkHandle {
    Arc::new(Mutex::new(ChartLink::new(Domain1D::new(x_min, x_max))))
}
