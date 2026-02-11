//! blinc_charts
//!
//! Canvas-first, GPU-accelerated, interactive charts for Blinc.
//!
//! Design goals (initial):
//! - Compose naturally with Blinc's layout tree (Stack overlays, Canvas rendering)
//! - Use Blinc's built-in event routing (mouse/touch/scroll/pinch/drag)
//! - Prioritize performance for large datasets via sampling/LOD and GPU pipelines

mod brush;
mod common;
mod link;
mod lod;
mod segments;
mod time_series;
mod view;

pub mod area;
pub mod bar;
pub mod candlestick;
pub mod contour;
pub mod density_map;
pub mod gauge;
pub mod geo;
pub mod heatmap;
pub mod hierarchy;
pub mod histogram;
pub mod line;
pub mod multi_line;
pub mod network;
pub mod polar;
pub mod scatter;
pub mod stacked_area;
pub mod statistics;

pub use brush::BrushX;
pub use candlestick::{Candle, CandleSeries};
pub use link::{chart_link, ChartLink, ChartLinkHandle};
pub use lod::{downsample_min_max, DownsampleParams};
pub use segments::runs_by_gap;
pub use time_series::TimeSeriesF32;
pub use view::{ChartView, Domain1D, Domain2D};

/// Common imports for chart users.
pub mod prelude {
    pub use crate::area::{
        area_chart, linked_area_chart, AreaChartHandle, AreaChartModel, AreaChartStyle,
    };
    pub use crate::bar::{
        bar_chart, linked_bar_chart, BarChartHandle, BarChartModel, BarChartStyle,
    };
    pub use crate::candlestick::{
        candlestick_chart, linked_candlestick_chart, Candle, CandleSeries, CandlestickChartHandle,
        CandlestickChartModel, CandlestickChartStyle,
    };
    pub use crate::contour::{
        contour_chart, ContourChartHandle, ContourChartModel, ContourChartStyle,
    };
    pub use crate::density_map::{
        density_map_chart, DensityMapChartHandle, DensityMapChartModel, DensityMapChartStyle,
    };
    pub use crate::gauge::{
        funnel_chart, gauge_chart, FunnelChartHandle, FunnelChartModel, FunnelChartStyle,
        GaugeChartHandle, GaugeChartModel, GaugeChartStyle,
    };
    pub use crate::geo::{geo_chart, GeoChartHandle, GeoChartModel, GeoChartStyle};
    pub use crate::heatmap::{
        heatmap_chart, HeatmapChartHandle, HeatmapChartModel, HeatmapChartStyle,
    };
    pub use crate::hierarchy::{
        hierarchy_chart, HierarchyChartHandle, HierarchyChartModel, HierarchyChartStyle,
        HierarchyMode, HierarchyNode,
    };
    pub use crate::histogram::{
        histogram_chart, HistogramChartHandle, HistogramChartModel, HistogramChartStyle,
    };
    pub use crate::line::{
        line_chart, linked_line_chart, LineChartHandle, LineChartModel, LineChartStyle,
    };
    pub use crate::link::{chart_link, ChartLink, ChartLinkHandle};
    pub use crate::multi_line::{
        linked_multi_line_chart, multi_line_chart, MultiLineChartHandle, MultiLineChartModel,
        MultiLineChartStyle,
    };
    pub use crate::network::{
        network_chart, NetworkChartHandle, NetworkChartModel, NetworkChartStyle, NetworkMode,
    };
    pub use crate::polar::{
        polar_chart, PolarChartHandle, PolarChartMode, PolarChartModel, PolarChartStyle,
    };
    pub use crate::scatter::{
        linked_scatter_chart, scatter_chart, ScatterChartHandle, ScatterChartModel,
        ScatterChartStyle,
    };
    pub use crate::stacked_area::{
        linked_stacked_area_chart, stacked_area_chart, StackedAreaChartHandle,
        StackedAreaChartModel, StackedAreaChartStyle, StackedAreaMode,
    };
    pub use crate::statistics::{
        statistics_chart, StatisticsChartHandle, StatisticsChartModel, StatisticsChartStyle,
    };
    pub use crate::time_series::TimeSeriesF32;
    pub use crate::view::{ChartView, Domain1D, Domain2D};
}
