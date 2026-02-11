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
pub mod input;
mod link;
mod lod;
mod segments;
mod time_series;
mod view;
mod xy_stack;

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
pub use input::{ChartInputBindings, DragAction, DragBinding, ModifiersReq};
pub use link::{chart_link, ChartLink, ChartLinkHandle};
pub use lod::{downsample_min_max, DownsampleParams};
pub use segments::runs_by_gap;
pub use time_series::TimeSeriesF32;
pub use view::{ChartView, Domain1D, Domain2D};

/// Common imports for chart users.
pub mod prelude {
    pub use crate::area::{
        area_chart, area_chart_with_bindings, linked_area_chart, linked_area_chart_with_bindings,
        AreaChartHandle, AreaChartModel, AreaChartStyle,
    };
    pub use crate::bar::{
        bar_chart, bar_chart_with_bindings, linked_bar_chart, linked_bar_chart_with_bindings,
        BarChartHandle, BarChartModel, BarChartStyle,
    };
    pub use crate::candlestick::{
        candlestick_chart, candlestick_chart_with_bindings, linked_candlestick_chart,
        linked_candlestick_chart_with_bindings, Candle, CandleSeries, CandlestickChartHandle,
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
        histogram_chart, histogram_chart_with_bindings, HistogramChartHandle, HistogramChartModel,
        HistogramChartStyle,
    };
    pub use crate::line::{
        line_chart, line_chart_with_bindings, linked_line_chart, linked_line_chart_with_bindings,
        LineChartHandle, LineChartModel, LineChartStyle,
    };
    pub use crate::link::{chart_link, ChartLink, ChartLinkHandle};
    pub use crate::multi_line::{
        linked_multi_line_chart, linked_multi_line_chart_with_bindings, multi_line_chart,
        multi_line_chart_with_bindings, MultiLineChartHandle, MultiLineChartModel,
        MultiLineChartStyle,
    };
    pub use crate::network::{
        network_chart, NetworkChartHandle, NetworkChartModel, NetworkChartStyle, NetworkMode,
    };
    pub use crate::polar::{
        polar_chart, PolarChartHandle, PolarChartMode, PolarChartModel, PolarChartStyle,
    };
    pub use crate::scatter::{
        linked_scatter_chart, linked_scatter_chart_with_bindings, scatter_chart,
        scatter_chart_with_bindings, ScatterChartHandle, ScatterChartModel, ScatterChartStyle,
    };
    pub use crate::stacked_area::{
        linked_stacked_area_chart, linked_stacked_area_chart_with_bindings, stacked_area_chart,
        stacked_area_chart_with_bindings, StackedAreaChartHandle, StackedAreaChartModel,
        StackedAreaChartStyle, StackedAreaMode,
    };
    pub use crate::statistics::{
        statistics_chart, statistics_chart_with_bindings, StatisticsChartHandle,
        StatisticsChartModel, StatisticsChartStyle,
    };
    pub use crate::time_series::TimeSeriesF32;
    pub use crate::view::{ChartView, Domain1D, Domain2D};
    pub use crate::{ChartInputBindings, DragAction, DragBinding, ModifiersReq};
}
