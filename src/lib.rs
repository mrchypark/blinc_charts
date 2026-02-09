//! blinc_charts
//!
//! Canvas-first, GPU-accelerated, interactive charts for Blinc.
//!
//! Design goals (initial):
//! - Compose naturally with Blinc's layout tree (Stack overlays, Canvas rendering)
//! - Use Blinc's built-in event routing (mouse/touch/scroll/pinch/drag)
//! - Prioritize performance for large datasets via sampling/LOD and GPU pipelines

mod lod;
mod time_series;
mod view;

pub mod line;

pub use lod::{downsample_min_max, DownsampleParams};
pub use time_series::TimeSeriesF32;
pub use view::{ChartView, Domain1D, Domain2D};

/// Common imports for chart users.
pub mod prelude {
    pub use crate::line::{line_chart, LineChartHandle, LineChartModel, LineChartStyle};
    pub use crate::time_series::TimeSeriesF32;
    pub use crate::view::{ChartView, Domain1D, Domain2D};
}
