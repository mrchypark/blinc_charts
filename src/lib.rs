//! blinc_charts
//!
//! Canvas-first, GPU-accelerated, interactive charts for Blinc.
//!
//! Design goals (initial):
//! - Compose naturally with Blinc's layout tree (Stack overlays, Canvas rendering)
//! - Use Blinc's built-in event routing (mouse/touch/scroll/pinch/drag)
//! - Prioritize performance for large datasets via sampling/LOD and GPU pipelines

mod lod;
mod brush;
mod link;
mod segments;
mod time_series;
mod view;

pub mod line;
pub mod multi_line;

pub use brush::BrushX;
pub use link::{chart_link, ChartLink, ChartLinkHandle};
pub use lod::{downsample_min_max, DownsampleParams};
pub use segments::runs_by_gap;
pub use time_series::TimeSeriesF32;
pub use view::{ChartView, Domain1D, Domain2D};

/// Common imports for chart users.
pub mod prelude {
    pub use crate::link::{chart_link, ChartLink, ChartLinkHandle};
    pub use crate::line::{
        line_chart, linked_line_chart, LineChartHandle, LineChartModel, LineChartStyle,
    };
    pub use crate::multi_line::{
        linked_multi_line_chart, multi_line_chart, MultiLineChartHandle, MultiLineChartModel,
        MultiLineChartStyle,
    };
    pub use crate::time_series::TimeSeriesF32;
    pub use crate::view::{ChartView, Domain1D, Domain2D};
}
