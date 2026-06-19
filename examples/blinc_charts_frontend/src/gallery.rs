use anyhow::Context;
use blinc_charts::prelude::*;
use blinc_core::Color;
use blinc_layout::prelude::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChartFamily {
    LinkedLine,
    LinkedArea,
    LinkedBar,
    Heatmap,
    Candlestick,
    Histogram,
    Statistics,
    Gauge,
    Funnel,
    Polar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChartSample {
    pub family: ChartFamily,
    pub title: &'static str,
    pub points: usize,
    pub summary: &'static str,
    pub interactions: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GalleryValidationReport {
    pub total_samples: usize,
    pub total_points: usize,
    pub linked_charts: usize,
}

pub fn sample_inventory() -> Vec<ChartSample> {
    vec![
        ChartSample {
            family: ChartFamily::LinkedLine,
            title: "Linked line",
            points: SERIES_POINTS,
            summary: "Shared X-domain line chart with hover, pan, zoom, and brush.",
            interactions: &[
                "hover X",
                "wheel/pinch zoom",
                "drag pan",
                "Shift+drag brush",
            ],
        },
        ChartSample {
            family: ChartFamily::LinkedArea,
            title: "Linked area",
            points: SERIES_POINTS,
            summary: "Filled linked series using the same interaction state.",
            interactions: &[
                "hover X",
                "wheel/pinch zoom",
                "drag pan",
                "Shift+drag brush",
            ],
        },
        ChartSample {
            family: ChartFamily::LinkedBar,
            title: "Linked bar",
            points: SERIES_POINTS,
            summary: "Grouped bars synchronized through a shared chart link.",
            interactions: &[
                "hover X",
                "wheel/pinch zoom",
                "drag pan",
                "Shift+drag brush",
            ],
        },
        ChartSample {
            family: ChartFamily::Heatmap,
            title: "Heatmap",
            points: HEATMAP_W * HEATMAP_H,
            summary: "Static grid render for inspecting cell color and budget.",
            interactions: &["static render", "cell budget"],
        },
        ChartSample {
            family: ChartFamily::Candlestick,
            title: "Candlestick",
            points: CANDLE_POINTS,
            summary: "OHLC candles with X-domain navigation and hover.",
            interactions: &[
                "hover candle",
                "wheel/pinch zoom",
                "drag pan",
                "Shift+drag brush",
            ],
        },
        ChartSample {
            family: ChartFamily::Histogram,
            title: "Histogram",
            points: HISTOGRAM_POINTS,
            summary: "Distribution bins with the shared X interaction model.",
            interactions: &[
                "hover bin",
                "wheel/pinch zoom",
                "drag pan",
                "Shift+drag brush",
            ],
        },
        ChartSample {
            family: ChartFamily::Statistics,
            title: "Statistics",
            points: STATS_GROUPS * STATS_POINTS_PER_GROUP,
            summary: "Grouped distribution summary with hover and X navigation.",
            interactions: &[
                "hover group",
                "wheel/pinch zoom",
                "drag pan",
                "Shift+drag brush",
            ],
        },
        ChartSample {
            family: ChartFamily::Gauge,
            title: "Gauge",
            points: 1,
            summary: "Indicator value rendered from host-controlled model state.",
            interactions: &["static render", "host update"],
        },
        ChartSample {
            family: ChartFamily::Funnel,
            title: "Funnel",
            points: FUNNEL_STAGES,
            summary: "Stage conversion view rendered from ordered model state.",
            interactions: &["static render", "stage normalization"],
        },
        ChartSample {
            family: ChartFamily::Polar,
            title: "Polar",
            points: POLAR_SERIES * POLAR_DIMS,
            summary: "Radar-style dimensions with hover-driven inspection.",
            interactions: &["hover dimension", "mode variants"],
        },
    ]
}

pub fn validate_sample_models() -> anyhow::Result<GalleryValidationReport> {
    let samples = sample_inventory();

    let linked = linked_series()?;
    let _line = LineChartHandle::new(LineChartModel::new(linked.line.clone()));
    let _area = AreaChartHandle::new(AreaChartModel::new(linked.area.clone()));
    let _bar = BarChartHandle::new(BarChartModel::new(linked.bar).context("bar sample")?);

    let _heatmap = HeatmapChartHandle::new(
        HeatmapChartModel::new(HEATMAP_W, HEATMAP_H, heatmap_values()).context("heatmap sample")?,
    );
    let _candlestick = CandlestickChartHandle::new(CandlestickChartModel::new(candle_series()?));
    let _histogram = HistogramChartHandle::new(
        HistogramChartModel::new(histogram_values()).context("histogram sample")?,
    );
    let mut statistics =
        StatisticsChartModel::new(statistics_groups()).context("statistics sample")?;
    statistics.style.mode = StatisticsMode::Violin;
    let _statistics = StatisticsChartHandle::new(statistics);
    let _gauge =
        GaugeChartHandle::new(GaugeChartModel::new(0.0, 100.0, 72.0).context("gauge sample")?);
    let _funnel =
        FunnelChartHandle::new(FunnelChartModel::new(funnel_stages()).context("funnel sample")?);
    let _polar = PolarChartHandle::new(
        PolarChartModel::new_radar(polar_dimensions(), polar_series()).context("polar sample")?,
    );

    Ok(GalleryValidationReport {
        total_samples: samples.len(),
        total_points: samples.iter().map(|sample| sample.points).sum(),
        linked_charts: samples
            .iter()
            .filter(|sample| {
                matches!(
                    sample.family,
                    ChartFamily::LinkedLine | ChartFamily::LinkedArea | ChartFamily::LinkedBar
                )
            })
            .count(),
    })
}

pub fn build_gallery_ui() -> anyhow::Result<Div> {
    let linked = linked_series()?;
    let link = chart_link(0.0, (SERIES_POINTS - 1) as f32);

    let mut line = LineChartModel::new(linked.line);
    line.style.line = Color::rgba(0.10, 0.72, 0.84, 1.0);
    line.style.stroke_width = 2.0;

    let mut area = AreaChartModel::new(linked.area);
    area.style.line = Color::rgba(0.95, 0.65, 0.20, 1.0);
    area.style.area = Color::rgba(0.95, 0.65, 0.20, 0.25);

    let mut bar = BarChartModel::new(linked.bar).context("bar chart")?;
    bar.style.stacked = false;
    bar.style.bar_alpha = 0.70;

    let heatmap =
        HeatmapChartModel::new(HEATMAP_W, HEATMAP_H, heatmap_values()).context("heatmap chart")?;
    let candles = CandlestickChartModel::new(candle_series()?);
    let histogram = HistogramChartModel::new(histogram_values()).context("histogram chart")?;
    let mut statistics =
        StatisticsChartModel::new(statistics_groups()).context("statistics chart")?;
    statistics.style.mode = StatisticsMode::Violin;
    let gauge = GaugeChartModel::new(0.0, 100.0, 72.0).context("gauge chart")?;
    let funnel = FunnelChartModel::new(funnel_stages()).context("funnel chart")?;
    let polar =
        PolarChartModel::new_radar(polar_dimensions(), polar_series()).context("polar chart")?;

    let content = div()
        .w_full()
        .h_fit()
        .flex_col()
        .p_px(18.0)
        .gap_px(14.0)
        .child(header())
        .child(section_label("Linked pan/zoom group"))
        .child(chart_card(
            "Line",
            interaction_hint(ChartFamily::LinkedLine),
            linked_line_chart(LineChartHandle::new(line), link.clone()),
        ))
        .child(chart_card(
            "Area",
            interaction_hint(ChartFamily::LinkedArea),
            linked_area_chart(AreaChartHandle::new(area), link.clone()),
        ))
        .child(chart_card(
            "Bar",
            interaction_hint(ChartFamily::LinkedBar),
            linked_bar_chart(BarChartHandle::new(bar), link),
        ))
        .child(section_label("Additional chart models"))
        .child(chart_card(
            "Heatmap",
            interaction_hint(ChartFamily::Heatmap),
            heatmap_chart(HeatmapChartHandle::new(heatmap)),
        ))
        .child(chart_card(
            "Candlestick",
            interaction_hint(ChartFamily::Candlestick),
            candlestick_chart(CandlestickChartHandle::new(candles)),
        ))
        .child(chart_card(
            "Histogram",
            interaction_hint(ChartFamily::Histogram),
            histogram_chart(HistogramChartHandle::new(histogram)),
        ))
        .child(chart_card(
            "Statistics",
            interaction_hint(ChartFamily::Statistics),
            statistics_chart(StatisticsChartHandle::new(statistics)),
        ))
        .child(chart_card(
            "Gauge",
            interaction_hint(ChartFamily::Gauge),
            gauge_chart(GaugeChartHandle::new(gauge)),
        ))
        .child(chart_card(
            "Funnel",
            interaction_hint(ChartFamily::Funnel),
            funnel_chart(FunnelChartHandle::new(funnel)),
        ))
        .child(chart_card(
            "Polar",
            interaction_hint(ChartFamily::Polar),
            polar_chart(PolarChartHandle::new(polar)),
        ));

    Ok(div()
        .w_full()
        .h_full()
        .bg(Color::rgba(0.04, 0.05, 0.07, 1.0))
        .child(scroll_no_bounce().w_full().h_full().child(content)))
}

fn header() -> Div {
    div()
        .w_full()
        .h_fit()
        .flex_col()
        .gap_px(6.0)
        .child(
            text("blinc_charts gallery")
                .size(28.0)
                .color(Color::rgba(0.95, 0.97, 1.0, 1.0)),
        )
        .child(
            text("Real Blinc layout elements backed by blinc_charts models and handles.")
                .size(14.0)
                .color(Color::rgba(0.72, 0.76, 0.82, 1.0)),
        )
}

fn section_label(label: &'static str) -> Div {
    div().w_full().h_fit().child(
        text(label)
            .size(16.0)
            .color(Color::rgba(0.84, 0.88, 0.94, 1.0)),
    )
}

fn chart_card(
    chart_title: &'static str,
    interaction: &'static str,
    chart: impl ElementBuilder + 'static,
) -> Div {
    div()
        .w_full()
        .h(322.0)
        .rounded(8.0)
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.10))
        .bg(Color::rgba(0.08, 0.09, 0.12, 1.0))
        .flex_col()
        .p_px(10.0)
        .gap_px(8.0)
        .child(
            text(chart_title)
                .size(13.0)
                .color(Color::rgba(0.82, 0.86, 0.92, 1.0)),
        )
        .child(
            text(interaction)
                .size(11.0)
                .color(Color::rgba(0.64, 0.70, 0.78, 1.0)),
        )
        .child(div().w_full().h(258.0).child(chart))
}

fn interaction_hint(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::LinkedLine | ChartFamily::LinkedArea | ChartFamily::LinkedBar => {
            "Linked: wheel/pinch zoom X, drag pan, Shift+drag brush, shared hover X."
        }
        ChartFamily::Candlestick | ChartFamily::Histogram | ChartFamily::Statistics => {
            "Interactive: wheel/pinch zoom X, drag pan, Shift+drag brush, hover nearest value."
        }
        ChartFamily::Polar => {
            "Hover dimensions to inspect the active axis; modes change interpretation."
        }
        ChartFamily::Gauge | ChartFamily::Funnel | ChartFamily::Heatmap => {
            "Static inspection: update model data or style, then rebuild/render."
        }
    }
}

struct LinkedSeries {
    line: TimeSeriesF32,
    area: TimeSeriesF32,
    bar: Vec<TimeSeriesF32>,
}

const SERIES_POINTS: usize = 160;
const HEATMAP_W: usize = 36;
const HEATMAP_H: usize = 20;
const CANDLE_POINTS: usize = 90;
const HISTOGRAM_POINTS: usize = 360;
const STATS_GROUPS: usize = 5;
const STATS_POINTS_PER_GROUP: usize = 48;
const FUNNEL_STAGES: usize = 5;
const POLAR_DIMS: usize = 6;
const POLAR_SERIES: usize = 3;

fn linked_series() -> anyhow::Result<LinkedSeries> {
    let x: Vec<f32> = (0..SERIES_POINTS).map(|idx| idx as f32).collect();

    let line_y: Vec<f32> = x
        .iter()
        .map(|x| 54.0 + wave(*x, 0.13, 14.0) + wave(*x, 0.031, 9.0))
        .collect();
    let area_y: Vec<f32> = x
        .iter()
        .map(|x| 34.0 + wave(*x, 0.08, 8.0) + (*x * 0.06).sin().abs() * 16.0)
        .collect();
    let bars_a: Vec<f32> = x
        .iter()
        .map(|x| 14.0 + ((*x * 0.18).sin() + 1.0) * 8.0)
        .collect();
    let bars_b: Vec<f32> = x
        .iter()
        .map(|x| 9.0 + ((*x * 0.11 + 1.3).cos() + 1.0) * 6.0)
        .collect();

    Ok(LinkedSeries {
        line: TimeSeriesF32::new(x.clone(), line_y)?,
        area: TimeSeriesF32::new(x.clone(), area_y)?,
        bar: vec![
            TimeSeriesF32::new(x.clone(), bars_a)?,
            TimeSeriesF32::new(x, bars_b)?,
        ],
    })
}

fn heatmap_values() -> Vec<f32> {
    let mut values = Vec::with_capacity(HEATMAP_W * HEATMAP_H);
    for y in 0..HEATMAP_H {
        for x in 0..HEATMAP_W {
            let ridge = ((x as f32 - 18.0).powi(2) * 0.018 + (y as f32 - 9.0).powi(2) * 0.055)
                .exp()
                .recip();
            values.push(ridge * 72.0 + wave(x as f32, 0.7, 10.0) + wave(y as f32, 0.9, 7.0));
        }
    }
    values
}

fn candle_series() -> anyhow::Result<CandleSeries> {
    let mut candles = Vec::with_capacity(CANDLE_POINTS);
    let mut close = 101.0;
    for idx in 0..CANDLE_POINTS {
        let drift = wave(idx as f32, 0.17, 1.8) + wave(idx as f32, 0.043, 2.6);
        let open = close;
        close = (close + drift).max(82.0);
        let high = open.max(close) + 1.2 + (idx as f32 * 0.31).sin().abs() * 2.0;
        let low = open.min(close) - 1.0 - (idx as f32 * 0.27).cos().abs() * 1.7;
        candles.push(Candle {
            x: idx as f32,
            open,
            high,
            low,
            close,
        });
    }
    CandleSeries::new(candles)
}

fn histogram_values() -> Vec<f32> {
    (0..HISTOGRAM_POINTS)
        .map(|idx| {
            let x = idx as f32;
            42.0 + wave(x, 0.23, 15.0) + wave(x, 0.071, 8.0) + ((idx % 17) as f32 - 8.0) * 0.9
        })
        .collect()
}

fn statistics_groups() -> Vec<Vec<f32>> {
    (0..STATS_GROUPS)
        .map(|group| {
            (0..STATS_POINTS_PER_GROUP)
                .map(|idx| {
                    let x = idx as f32;
                    20.0 + group as f32 * 8.0
                        + wave(x, 0.22 + group as f32 * 0.01, 6.0)
                        + wave(x, 0.053, 3.0)
                })
                .collect()
        })
        .collect()
}

fn funnel_stages() -> Vec<(String, f32)> {
    [
        ("Visitors", 12_000.0),
        ("Trials", 5_600.0),
        ("Activated", 3_250.0),
        ("Paid", 1_480.0),
        ("Retained", 1_040.0),
    ]
    .into_iter()
    .map(|(label, value)| (label.to_string(), value))
    .collect()
}

fn polar_dimensions() -> Vec<String> {
    ["Speed", "Quality", "Cost", "Reach", "Risk", "Retention"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn polar_series() -> Vec<Vec<f32>> {
    vec![
        vec![0.82, 0.68, 0.54, 0.77, 0.42, 0.70],
        vec![0.62, 0.86, 0.73, 0.58, 0.60, 0.81],
        vec![0.74, 0.59, 0.66, 0.84, 0.48, 0.64],
    ]
}

fn wave(x: f32, freq: f32, amp: f32) -> f32 {
    (x * freq).sin() * amp
}
