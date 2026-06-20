use anyhow::Context;
use blinc_app::windowed::WindowedContext;
use blinc_app::WindowConfig;
use blinc_charts::prelude::*;
use blinc_core::{Color, Point, State};
use blinc_layout::prelude::*;

const PAGE_SCROLL_ZOOM_FACTOR: f32 = 0.0;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChartFamily {
    #[default]
    LinkedLine,
    LinkedArea,
    LinkedBar,
    MultiLine,
    StackedArea,
    Scatter,
    Candlestick,
    Histogram,
    Statistics,
    Heatmap,
    Contour,
    DensityMap,
    Gauge,
    Funnel,
    Polar,
    Geo,
    Hierarchy,
    Network,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GalleryTab {
    #[default]
    Example,
    Code,
    Variants,
    Interactions,
    Explanation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GalleryGroup {
    Linked,
    TimeSeries,
    Distribution,
    Surface,
    Structure,
    Indicator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartSample {
    pub family: ChartFamily,
    pub title: &'static str,
    pub points: usize,
    pub summary: &'static str,
    pub interactions: &'static [&'static str],
    pub code_snippet: &'static str,
    pub variants: &'static [(&'static str, &'static str, &'static str)],
    pub interaction_variants: &'static [(&'static str, &'static str, &'static str)],
    pub explanation: &'static [(&'static str, &'static [&'static str])],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GalleryValidationReport {
    pub total_samples: usize,
    pub total_points: usize,
    pub linked_charts: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionDemo {
    pub title: &'static str,
    pub instruction: &'static str,
    pub code_change: &'static str,
    pub effect: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageCase {
    pub family: ChartFamily,
    pub variant: &'static str,
    pub variant_code: &'static str,
    pub variant_effect: &'static str,
    pub interaction: &'static str,
    pub interaction_code: &'static str,
    pub interaction_effect: &'static str,
    pub task: String,
    pub evidence: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InteractionDemoKind {
    XPanAndHover,
    XShiftBrush,
    XDragBrush,
    XLinkedSync,
    TwoDimensional,
    HoverOnly,
    StaticModel,
}

#[derive(Clone, Copy, Debug)]
struct InteractionDemoEntry {
    spec: InteractionDemo,
    kind: InteractionDemoKind,
}

#[derive(Clone, Copy, Debug)]
struct GalleryExample {
    family: ChartFamily,
    group: GalleryGroup,
    title: &'static str,
    summary: &'static str,
    points: usize,
    api: &'static [&'static str],
    interactions: &'static [&'static str],
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
const SURFACE_W: usize = 40;
const SURFACE_H: usize = 26;

const EXAMPLES: &[GalleryExample] = &[
    GalleryExample {
        family: ChartFamily::LinkedLine,
        group: GalleryGroup::Linked,
        title: "Linked line",
        summary: "Time-series line chart with hover, LOD, X zoom, pan, brush, and linked domains.",
        points: SERIES_POINTS,
        api: &["LineChartModel", "LineChartHandle", "linked_line_chart", "chart_link"],
        interactions: &["hover nearest point", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::LinkedArea,
        group: GalleryGroup::Linked,
        title: "Linked area",
        summary: "Filled time-series chart that shares the same X-domain interaction model as line.",
        points: SERIES_POINTS,
        api: &["AreaChartModel", "AreaChartStyle", "linked_area_chart"],
        interactions: &["hover point", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::LinkedBar,
        group: GalleryGroup::Linked,
        title: "Linked bar",
        summary: "Grouped or stacked bars over a continuous X-domain, including negative-domain handling.",
        points: SERIES_POINTS * 2,
        api: &["BarChartModel", "BarChartStyle", "linked_bar_chart"],
        interactions: &["hover X", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::MultiLine,
        group: GalleryGroup::TimeSeries,
        title: "Multi-line",
        summary: "Many related series with gap splitting, per-series LOD, and density fallback under budgets.",
        points: SERIES_POINTS * 5,
        api: &["MultiLineChartModel", "MultiLineChartStyle", "multi_line_chart"],
        interactions: &["hover X", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::StackedArea,
        group: GalleryGroup::TimeSeries,
        title: "Stacked area",
        summary: "Layered area bands for part-to-whole time series, with stacked and streamgraph modes.",
        points: SERIES_POINTS * 4,
        api: &["StackedAreaChartModel", "StackedAreaMode", "stacked_area_chart"],
        interactions: &["hover X", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::Scatter,
        group: GalleryGroup::TimeSeries,
        title: "Scatter",
        summary: "Point cloud over an X/Y domain with spatial-index hover and draw-budget controls.",
        points: SERIES_POINTS * 2,
        api: &["ScatterChartModel", "ScatterChartStyle", "scatter_chart"],
        interactions: &["hover nearest point", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::Candlestick,
        group: GalleryGroup::TimeSeries,
        title: "Candlestick",
        summary: "OHLC finance chart with visible-domain candle binning and up/down styling.",
        points: CANDLE_POINTS,
        api: &["Candle", "CandleSeries", "CandlestickChartModel"],
        interactions: &["hover X", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::Histogram,
        group: GalleryGroup::Distribution,
        title: "Histogram",
        summary: "Distribution bins with dynamic visible X-domain and configurable bin counts.",
        points: HISTOGRAM_POINTS,
        api: &["HistogramChartModel", "HistogramChartStyle", "histogram_chart"],
        interactions: &["hover bin", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::Statistics,
        group: GalleryGroup::Distribution,
        title: "Statistics",
        summary: "Grouped statistical summaries rendered as boxplot, violin, or error-band views.",
        points: STATS_GROUPS * STATS_POINTS_PER_GROUP,
        api: &["StatisticsChartModel", "StatisticsMode", "statistics_chart"],
        interactions: &["hover group", "pinch X zoom", "drag pan", "Shift+drag brush"],
    },
    GalleryExample {
        family: ChartFamily::Heatmap,
        group: GalleryGroup::Surface,
        title: "Heatmap",
        summary: "Static matrix chart for dense rectangular values with screen-cell budget limits.",
        points: HEATMAP_W * HEATMAP_H,
        api: &["HeatmapChartModel", "HeatmapChartStyle", "heatmap_chart"],
        interactions: &["static render", "cell budget"],
    },
    GalleryExample {
        family: ChartFamily::Contour,
        group: GalleryGroup::Surface,
        title: "Contour",
        summary: "Iso-line rendering over a grid with 2D domain navigation and rectangular selection.",
        points: SURFACE_W * SURFACE_H,
        api: &["ContourChartModel", "ContourChartStyle", "contour_chart"],
        interactions: &["hover value", "2D zoom", "2D pan", "Shift+drag rectangle"],
    },
    GalleryExample {
        family: ChartFamily::DensityMap,
        group: GalleryGroup::Surface,
        title: "Density map",
        summary: "Binned density visualization for many XY points, computed over the visible domain.",
        points: 900,
        api: &["DensityMapChartModel", "DensityMapChartStyle", "density_map_chart"],
        interactions: &["hover bin count", "2D zoom", "2D pan", "Shift+drag rectangle"],
    },
    GalleryExample {
        family: ChartFamily::Gauge,
        group: GalleryGroup::Indicator,
        title: "Gauge",
        summary: "Single-value progress display with clamped values and transition support.",
        points: 1,
        api: &["GaugeChartModel", "GaugeChartStyle", "gauge_chart"],
        interactions: &["animated value transition", "range clamp"],
    },
    GalleryExample {
        family: ChartFamily::Funnel,
        group: GalleryGroup::Indicator,
        title: "Funnel",
        summary: "Stage conversion chart that normalizes widths against the largest positive stage.",
        points: FUNNEL_STAGES,
        api: &["FunnelChartModel", "FunnelChartStyle", "funnel_chart"],
        interactions: &["static render", "stage normalization"],
    },
    GalleryExample {
        family: ChartFamily::Polar,
        group: GalleryGroup::Indicator,
        title: "Polar / Radar",
        summary: "Multivariate radial chart with radar, polar, and parallel coordinate modes.",
        points: POLAR_SERIES * POLAR_DIMS,
        api: &["PolarChartModel", "PolarChartMode", "polar_chart"],
        interactions: &["hover dimension", "mode variants"],
    },
    GalleryExample {
        family: ChartFamily::Geo,
        group: GalleryGroup::Surface,
        title: "Geo",
        summary: "Projected shape outlines over a 2D domain with pan, zoom, hover coordinate, and point budgets.",
        points: 28,
        api: &["GeoChartModel", "GeoChartStyle", "geo_chart"],
        interactions: &["hover coordinate", "2D zoom", "2D pan", "draw budget"],
    },
    GalleryExample {
        family: ChartFamily::Hierarchy,
        group: GalleryGroup::Structure,
        title: "Hierarchy",
        summary: "Tree data rendered as treemap, icicle, sunburst, or packing layouts.",
        points: 13,
        api: &["HierarchyNode", "HierarchyChartModel", "HierarchyMode"],
        interactions: &["hover leaf", "layout mode variants", "leaf cap"],
    },
    GalleryExample {
        family: ChartFamily::Network,
        group: GalleryGroup::Structure,
        title: "Network",
        summary: "Relationship data rendered as graph, Sankey, or chord diagrams with hover and graph navigation.",
        points: 8,
        api: &["NetworkChartModel", "NetworkMode", "network_chart"],
        interactions: &["hover node", "graph pan/zoom", "mode variants", "node/link caps"],
    },
];

const GROUPS: &[GalleryGroup] = &[
    GalleryGroup::Linked,
    GalleryGroup::TimeSeries,
    GalleryGroup::Distribution,
    GalleryGroup::Surface,
    GalleryGroup::Structure,
    GalleryGroup::Indicator,
];

const TABS: &[GalleryTab] = &[
    GalleryTab::Example,
    GalleryTab::Code,
    GalleryTab::Variants,
    GalleryTab::Explanation,
];

pub fn sample_inventory() -> Vec<ChartSample> {
    EXAMPLES
        .iter()
        .map(|example| ChartSample {
            family: example.family,
            title: example.title,
            points: example.points,
            summary: example.summary,
            interactions: example.interactions,
            code_snippet: code_snippet(example.family),
            variants: variant_notes(example.family),
            interaction_variants: interaction_notes(example.family),
            explanation: explanation_notes(example.family),
        })
        .collect()
}

pub fn interaction_demo_inventory(family: ChartFamily) -> Vec<InteractionDemo> {
    interaction_demo_entries(family)
        .into_iter()
        .map(|entry| entry.spec)
        .collect()
}

pub fn gallery_tab_labels() -> Vec<&'static str> {
    TABS.iter().map(|tab| tab.label()).collect()
}

pub fn coverage_matrix() -> Vec<CoverageCase> {
    let mut cases = Vec::new();
    for sample in sample_inventory() {
        for variant in sample.variants {
            for interaction in interaction_demo_inventory(sample.family) {
                cases.push(CoverageCase {
                    family: sample.family,
                    variant: variant.0,
                    variant_code: variant.1,
                    variant_effect: variant.2,
                    interaction: interaction.title,
                    interaction_code: interaction.code_change,
                    interaction_effect: interaction.effect,
                    task: format!(
                        "Using only the provided blinc_charts examples, write a Rust function that builds chart={} variant={} interaction={} and returns a Blinc element.",
                        sample.title, variant.0, interaction.title
                    ),
                    evidence: format!(
                        "{}\n{}\n{}\n{}\n{}",
                        sample.code_snippet, variant.1, variant.2, interaction.code_change, interaction.effect
                    ),
                });
            }
        }
    }
    cases
}

pub fn build_interaction_examples_ui(family: ChartFamily) -> anyhow::Result<Div> {
    let example = example(family).expect("default gallery example exists");
    example_tab(example, build_chart(family)?, 624.0, None)
}

pub fn validate_sample_models() -> anyhow::Result<GalleryValidationReport> {
    for example in EXAMPLES {
        build_chart(example.family).with_context(|| format!("{} sample", example.title))?;
    }

    Ok(GalleryValidationReport {
        total_samples: EXAMPLES.len(),
        total_points: EXAMPLES.iter().map(|sample| sample.points).sum(),
        linked_charts: EXAMPLES
            .iter()
            .filter(|sample| sample.group == GalleryGroup::Linked)
            .count(),
    })
}

pub fn desktop_window_config() -> WindowConfig {
    WindowConfig::new("blinc_charts desktop gallery")
        .size(1280, 860)
        .resizable(true)
        .min_size(900, 640)
}

pub fn build_native_ui(ctx: &mut WindowedContext) -> Div {
    let selected = ctx.use_state_keyed("gallery.selected_chart", ChartFamily::default);
    let active_tab = ctx.use_state_keyed("gallery.active_tab", GalleryTab::default);
    let open_code = ctx.use_state_keyed("gallery.open_code_panel", String::new);

    build_desktop_ui_with_state(
        ctx.width,
        ctx.height,
        Some(selected),
        Some(active_tab),
        Some(open_code),
        "blinc_charts desktop gallery",
        "Native Blinc window backed by chart models, handles, variants, and code notes.",
    )
    .unwrap_or_else(|e| error_view(ctx.width, ctx.height, e))
}

pub fn build_desktop_ui(width: f32, height: f32) -> anyhow::Result<Div> {
    build_desktop_ui_with_state(
        width,
        height,
        None,
        None,
        None,
        "blinc_charts desktop gallery",
        "Native Blinc window backed by chart models, handles, variants, and code notes.",
    )
}

fn build_desktop_ui_with_state(
    width: f32,
    height: f32,
    selected_state: Option<State<ChartFamily>>,
    tab_state: Option<State<GalleryTab>>,
    open_code_state: Option<State<String>>,
    title: &'static str,
    subtitle: &'static str,
) -> anyhow::Result<Div> {
    let selected = selected_state.as_ref().map(State::get).unwrap_or_default();
    let active_tab = tab_state.as_ref().map(State::get).unwrap_or_default();
    let selected = if example(selected).is_some() {
        selected
    } else {
        ChartFamily::default()
    };
    let example = example(selected).expect("default gallery example exists");

    Ok(div()
        .w(width)
        .h(height)
        .bg(Color::rgba(0.045, 0.050, 0.060, 1.0))
        .flex_row()
        .child(sidebar(
            selected,
            selected_state.clone(),
            tab_state.clone(),
            title,
            height,
        ))
        .child(detail_pane(
            example,
            active_tab,
            tab_state,
            open_code_state,
            subtitle,
            build_chart(selected)?,
            height,
        )?))
}

pub fn build_gallery_ui() -> anyhow::Result<Div> {
    build_desktop_ui(1280.0, 860.0)
}

fn sidebar(
    selected: ChartFamily,
    selected_state: Option<State<ChartFamily>>,
    tab_state: Option<State<GalleryTab>>,
    title: &'static str,
    height: f32,
) -> Scroll {
    let mut content = div()
        .w_full()
        .h_fit()
        .flex_col()
        .gap_px(14.0)
        .p_px(16.0)
        .child(
            div()
                .w_full()
                .h_fit()
                .flex_col()
                .gap_px(6.0)
                .child(
                    text(title)
                        .size(22.0)
                        .color(Color::rgba(0.96, 0.98, 1.0, 1.0)),
                )
                .child(
                    text("Reference gallery")
                        .size(12.0)
                        .color(Color::rgba(0.66, 0.72, 0.78, 1.0)),
                ),
        );

    for group in GROUPS {
        content = content.child(group_label(group.label())).child(group_items(
            *group,
            selected,
            selected_state.clone(),
            tab_state.clone(),
        ));
    }

    scroll_no_bounce()
        .w(278.0)
        .h(height)
        .vertical()
        .bg(Color::rgba(0.070, 0.076, 0.090, 1.0))
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.08))
        .child(content)
}

fn group_items(
    group: GalleryGroup,
    selected: ChartFamily,
    selected_state: Option<State<ChartFamily>>,
    tab_state: Option<State<GalleryTab>>,
) -> Div {
    let mut items = div().w_full().h_fit().flex_col().gap_px(4.0);

    for item in EXAMPLES.iter().filter(|item| item.group == group) {
        let is_active = selected == item.family;
        let selected_state_for_click = selected_state.clone();
        let tab_state_for_click = tab_state.clone();
        let family = item.family;

        items = items.child(
            div()
                .w_full()
                .h(34.0)
                .rounded(7.0)
                .cursor_pointer()
                .items_center()
                .p_px(8.0)
                .bg(if is_active {
                    Color::rgba(0.18, 0.25, 0.34, 1.0)
                } else {
                    Color::rgba(0.09, 0.10, 0.12, 0.0)
                })
                .border(
                    1.0,
                    if is_active {
                        Color::rgba(0.50, 0.74, 0.90, 0.40)
                    } else {
                        Color::rgba(1.0, 1.0, 1.0, 0.00)
                    },
                )
                .child(text(item.title).size(12.0).color(if is_active {
                    Color::rgba(0.94, 0.98, 1.0, 1.0)
                } else {
                    Color::rgba(0.70, 0.75, 0.82, 1.0)
                }))
                .on_click(move |_| {
                    if let Some(state) = selected_state_for_click.as_ref() {
                        state.set_rebuild(family);
                    }
                    if let Some(state) = tab_state_for_click.as_ref() {
                        state.set_rebuild(GalleryTab::Example);
                    }
                }),
        );
    }

    items
}

fn detail_pane(
    example: &GalleryExample,
    active_tab: GalleryTab,
    tab_state: Option<State<GalleryTab>>,
    open_code_state: Option<State<String>>,
    subtitle: &'static str,
    chart: Div,
    height: f32,
) -> anyhow::Result<Div> {
    let active_tab_for_bar = match active_tab {
        GalleryTab::Interactions => GalleryTab::Example,
        tab => tab,
    };
    let content_height = (height - 236.0).max(320.0);
    let content = match active_tab {
        GalleryTab::Example | GalleryTab::Interactions => {
            example_tab(example, chart, content_height, open_code_state)?
        }
        GalleryTab::Code => code_tab(example.family, content_height),
        GalleryTab::Variants => variants_tab(example.family, content_height),
        GalleryTab::Explanation => explanation_tab(example.family, content_height),
    };

    Ok(div()
        .flex_1()
        .h_full()
        .flex_col()
        .p_px(18.0)
        .gap_px(14.0)
        .child(detail_header(example, subtitle))
        .child(tab_bar(active_tab_for_bar, tab_state))
        .child(content))
}

fn detail_header(example: &GalleryExample, subtitle: &'static str) -> Div {
    div()
        .w_full()
        .h_fit()
        .flex_col()
        .gap_px(9.0)
        .child(
            div()
                .w_full()
                .h_fit()
                .flex_row()
                .items_center()
                .gap_px(8.0)
                .child(
                    text(example.title)
                        .size(27.0)
                        .color(Color::rgba(0.96, 0.98, 1.0, 1.0)),
                )
                .child(metric_chip(&format!("{} pts", example.points))),
        )
        .child(
            text(subtitle)
                .size(12.0)
                .color(Color::rgba(0.62, 0.68, 0.74, 1.0)),
        )
        .child(
            text(example.summary)
                .size(14.0)
                .color(Color::rgba(0.80, 0.84, 0.89, 1.0)),
        )
        .child(chips(example.interactions))
        .child(chips(example.api))
}

fn tab_bar(active_tab: GalleryTab, tab_state: Option<State<GalleryTab>>) -> Div {
    let mut row = div().w_full().h(40.0).flex_row().gap_px(6.0).items_center();

    for tab in TABS {
        let is_active = *tab == active_tab;
        let tab_state_for_click = tab_state.clone();
        let next_tab = *tab;
        row = row.child(
            div()
                .h(34.0)
                .w(112.0)
                .rounded(7.0)
                .items_center()
                .justify_center()
                .cursor_pointer()
                .bg(if is_active {
                    Color::rgba(0.84, 0.58, 0.20, 1.0)
                } else {
                    Color::rgba(0.10, 0.11, 0.13, 1.0)
                })
                .border(
                    1.0,
                    if is_active {
                        Color::rgba(1.0, 0.82, 0.42, 0.55)
                    } else {
                        Color::rgba(1.0, 1.0, 1.0, 0.09)
                    },
                )
                .child(text(tab.label()).size(12.0).color(if is_active {
                    Color::rgba(0.08, 0.07, 0.05, 1.0)
                } else {
                    Color::rgba(0.76, 0.80, 0.86, 1.0)
                }))
                .on_click(move |_| {
                    if let Some(state) = tab_state_for_click.as_ref() {
                        state.set_rebuild(next_tab);
                    }
                }),
        );
    }

    row
}

fn example_tab(
    example: &GalleryExample,
    chart: Div,
    content_height: f32,
    open_code_state: Option<State<String>>,
) -> anyhow::Result<Div> {
    let panel = div()
        .w_full()
        .h_fit()
        .flex_col()
        .gap_px(12.0)
        .p_px(2.0)
        .child(main_example_card(
            example.family,
            chart,
            open_code_state.clone(),
        ))
        .child(info_grid(&[
            ("Data shape", data_shape(example.family)),
            ("What to try", interaction_hint(example.family)),
            ("Budget note", budget_note(example.family)),
        ]))
        .child(section_heading("Runnable interaction examples"))
        .child(interaction_examples_panel(example.family, open_code_state)?);

    Ok(scroll_tab(content_height, panel))
}

fn main_example_card(
    family: ChartFamily,
    chart: Div,
    open_code_state: Option<State<String>>,
) -> Div {
    div()
        .w_full()
        .h_fit()
        .rounded(8.0)
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.10))
        .bg(Color::rgba(0.075, 0.083, 0.100, 1.0))
        .p_px(10.0)
        .flex_col()
        .gap_px(10.0)
        .child(div().w_full().h(380.0).child(chart))
        .child(collapsible_code_panel(
            "main-example",
            "Example code",
            code_snippet(family),
            open_code_state,
        ))
}

fn scroll_tab(content_height: f32, content: Div) -> Div {
    div().w_full().flex_1().child(
        scroll_no_bounce()
            .w_full()
            .h(content_height)
            .vertical()
            .child(content),
    )
}

fn code_tab(family: ChartFamily, content_height: f32) -> Div {
    let panel = div()
        .w_full()
        .h_fit()
        .flex_col()
        .gap_px(12.0)
        .child(section_heading("Minimal Rust setup"))
        .child(
            code(code_snippet(family))
                .line_numbers(true)
                .font_size(12.0)
                .code_bg(Color::rgba(0.035, 0.040, 0.048, 1.0))
                .rounded(8.0),
        )
        .child(section_heading("Code layers"))
        .child(info_grid(&[
            ("Model", constructor_note(family)),
            ("Variant", "Change one style or mode field, then compare the rendered effect."),
            ("Binding", "Change ChartInputBindings only when the gesture contract changes."),
        ]))
        .child(info_grid(&[
            ("Constructor", constructor_note(family)),
            ("Handle", "Wrap the model in the matching *ChartHandle so Blinc can render and mutate it safely."),
            ("Element", "Pass the handle to *_chart(handle), or to linked_*_chart(handle, chart_link) for shared X state."),
        ]));

    scroll_tab(content_height, panel)
}

fn variants_tab(family: ChartFamily, content_height: f32) -> Div {
    let mut panel = div()
        .w_full()
        .h_fit()
        .flex_col()
        .gap_px(10.0)
        .child(section_heading("Code variance meanings"));
    for variant in variant_notes(family) {
        panel = panel.child(variant_card(variant.0, variant.1, variant.2));
    }
    scroll_tab(content_height, panel)
}

fn interaction_examples_panel(
    family: ChartFamily,
    open_code_state: Option<State<String>>,
) -> anyhow::Result<Div> {
    let mut panel = div().w_full().h_fit().flex_col().gap_px(14.0).p_px(2.0);

    for entry in interaction_demo_entries(family) {
        panel = panel.child(interaction_demo_card(
            entry.spec,
            interaction_demo_chart(family, entry.kind)
                .with_context(|| format!("{} interaction demo", entry.spec.title))?,
            open_code_state.clone(),
        ));
    }

    Ok(panel)
}

fn explanation_tab(family: ChartFamily, content_height: f32) -> Div {
    let mut panel = div()
        .w_full()
        .h_fit()
        .flex_col()
        .gap_px(12.0)
        .child(section_heading("How to read this chart"));

    for (title, body) in explanation_notes(family) {
        panel = panel.child(body_block(title, body));
    }
    scroll_tab(content_height, panel)
}

fn build_chart(family: ChartFamily) -> anyhow::Result<Div> {
    Ok(match family {
        ChartFamily::LinkedLine => {
            let linked = linked_series()?;
            let link = chart_link(0.0, (SERIES_POINTS - 1) as f32);
            let mut line = LineChartModel::new(linked.line);
            line.style.line = Color::rgba(0.10, 0.72, 0.84, 1.0);
            line.style.stroke_width = 2.0;
            chart_surface(linked_line_chart_with_bindings(
                LineChartHandle::new(line),
                link,
                page_chart_bindings(),
            ))
        }
        ChartFamily::LinkedArea => {
            let linked = linked_series()?;
            let link = chart_link(0.0, (SERIES_POINTS - 1) as f32);
            let mut area = AreaChartModel::new(linked.area);
            area.style.line = Color::rgba(0.95, 0.65, 0.20, 1.0);
            area.style.area = Color::rgba(0.95, 0.65, 0.20, 0.25);
            chart_surface(linked_area_chart_with_bindings(
                AreaChartHandle::new(area),
                link,
                page_chart_bindings(),
            ))
        }
        ChartFamily::LinkedBar => {
            let linked = linked_series()?;
            let link = chart_link(0.0, (SERIES_POINTS - 1) as f32);
            let mut bar = BarChartModel::new(linked.bar).context("bar chart")?;
            bar.style.stacked = false;
            bar.style.bar_alpha = 0.70;
            chart_surface(linked_bar_chart_with_bindings(
                BarChartHandle::new(bar),
                link,
                page_chart_bindings(),
            ))
        }
        ChartFamily::MultiLine => {
            let mut model = MultiLineChartModel::new(multi_line_series()?)?;
            model.set_gap_dx(9.0);
            chart_surface(multi_line_chart_with_bindings(
                MultiLineChartHandle::new(model),
                page_chart_bindings(),
            ))
        }
        ChartFamily::StackedArea => {
            let mut model = StackedAreaChartModel::new(stacked_area_series()?)?;
            model.style.mode = StackedAreaMode::Streamgraph;
            chart_surface(stacked_area_chart_with_bindings(
                StackedAreaChartHandle::new(model),
                page_chart_bindings(),
            ))
        }
        ChartFamily::Scatter => {
            let mut model = ScatterChartModel::new(scatter_series()?);
            model.set_max_points(1_200);
            chart_surface(scatter_chart_with_bindings(
                ScatterChartHandle::new(model),
                page_chart_bindings(),
            ))
        }
        ChartFamily::Candlestick => chart_surface(candlestick_chart_with_bindings(
            CandlestickChartHandle::new(CandlestickChartModel::new(candle_series()?)),
            page_chart_bindings(),
        )),
        ChartFamily::Histogram => chart_surface(histogram_chart_with_bindings(
            HistogramChartHandle::new(
                HistogramChartModel::new(histogram_values()).context("histogram chart")?,
            ),
            page_chart_bindings(),
        )),
        ChartFamily::Statistics => {
            let mut model =
                StatisticsChartModel::new(statistics_groups()).context("statistics chart")?;
            model.style.mode = StatisticsMode::Violin;
            chart_surface(statistics_chart_with_bindings(
                StatisticsChartHandle::new(model),
                page_chart_bindings(),
            ))
        }
        ChartFamily::Heatmap => chart_surface(heatmap_chart(HeatmapChartHandle::new(
            HeatmapChartModel::new(HEATMAP_W, HEATMAP_H, heatmap_values())
                .context("heatmap chart")?,
        ))),
        ChartFamily::Contour => {
            let mut model = ContourChartModel::new(SURFACE_W, SURFACE_H, surface_values())
                .context("contour chart")?;
            model.style.levels = vec![-18.0, -6.0, 6.0, 18.0, 30.0];
            model.style.scroll_zoom_factor = PAGE_SCROLL_ZOOM_FACTOR;
            chart_surface(contour_chart(ContourChartHandle::new(model)))
        }
        ChartFamily::DensityMap => {
            let mut model =
                DensityMapChartModel::new(density_points()).context("density map chart")?;
            model.style.scroll_zoom_factor = PAGE_SCROLL_ZOOM_FACTOR;
            chart_surface(density_map_chart(DensityMapChartHandle::new(model)))
        }
        ChartFamily::Gauge => chart_surface(gauge_chart(GaugeChartHandle::new(
            GaugeChartModel::new(0.0, 100.0, 72.0).context("gauge chart")?,
        ))),
        ChartFamily::Funnel => chart_surface(funnel_chart(FunnelChartHandle::new(
            FunnelChartModel::new(funnel_stages()).context("funnel chart")?,
        ))),
        ChartFamily::Polar => {
            let mut model = PolarChartModel::new_radar(polar_dimensions(), polar_series())
                .context("polar chart")?;
            model.mode = PolarChartMode::Radar;
            chart_surface(polar_chart(PolarChartHandle::new(model)))
        }
        ChartFamily::Geo => {
            let mut model = GeoChartModel::new(geo_shapes()).context("geo chart")?;
            model.style.scroll_zoom_factor = PAGE_SCROLL_ZOOM_FACTOR;
            chart_surface(geo_chart(GeoChartHandle::new(model)))
        }
        ChartFamily::Hierarchy => {
            let mut model =
                HierarchyChartModel::new(hierarchy_root()).context("hierarchy chart")?;
            model.style.mode = HierarchyMode::Treemap;
            chart_surface(hierarchy_chart(HierarchyChartHandle::new(model)))
        }
        ChartFamily::Network => {
            let mut model = NetworkChartModel::new_sankey(network_nodes(), network_links())
                .context("network chart")?;
            model.style.scroll_zoom_factor = PAGE_SCROLL_ZOOM_FACTOR;
            chart_surface(network_chart(NetworkChartHandle::new(model)))
        }
    })
}

fn chart_surface(chart: impl ElementBuilder + 'static) -> Div {
    div().w_full().h_full().child(chart)
}

fn page_chart_bindings() -> ChartInputBindings {
    ChartInputBindings {
        scroll_zoom: false,
        ..ChartInputBindings::default()
    }
}

fn framed_chart(chart: impl ElementBuilder + 'static) -> Div {
    div()
        .w_full()
        .h(240.0)
        .rounded(8.0)
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.09))
        .bg(Color::rgba(0.050, 0.056, 0.068, 1.0))
        .p_px(8.0)
        .child(chart)
}

fn interaction_demo_card(
    spec: InteractionDemo,
    chart: Div,
    open_code_state: Option<State<String>>,
) -> Div {
    div()
        .w_full()
        .h_fit()
        .rounded(8.0)
        .bg(Color::rgba(0.070, 0.078, 0.094, 1.0))
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.08))
        .p_px(12.0)
        .flex_col()
        .gap_px(10.0)
        .child(
            div()
                .w_full()
                .h_fit()
                .flex_col()
                .gap_px(5.0)
                .child(
                    text(spec.title)
                        .size(16.0)
                        .color(Color::rgba(0.94, 0.97, 1.0, 1.0)),
                )
                .child(
                    text(spec.instruction)
                        .size(12.0)
                        .color(Color::rgba(0.74, 0.79, 0.85, 1.0)),
                ),
        )
        .child(framed_chart(chart))
        .child(collapsible_code_panel(
            spec.title,
            "Code",
            spec.code_change,
            open_code_state,
        ))
        .child(
            text(spec.effect)
                .size(12.0)
                .color(Color::rgba(0.74, 0.79, 0.85, 1.0)),
        )
}

fn collapsible_code_panel(
    panel_id: &'static str,
    label: &'static str,
    snippet: &'static str,
    open_code_state: Option<State<String>>,
) -> Div {
    let is_open = open_code_state
        .as_ref()
        .map(|state| state.get() == panel_id)
        .unwrap_or(false);
    let mut panel = div().w_full().h_fit().flex_col().gap_px(8.0).child(
        code_toggle_row(panel_id, label, is_open, open_code_state.clone()),
    );

    if is_open {
        panel = panel.child(
            code(snippet)
                .line_numbers(false)
                .font_size(12.0)
                .code_bg(Color::rgba(0.035, 0.040, 0.048, 1.0))
                .rounded(8.0),
        );
    }

    panel
}

fn code_toggle_row(
    panel_id: &'static str,
    label: &'static str,
    is_open: bool,
    open_code_state: Option<State<String>>,
) -> Div {
    let mut row = div()
        .w_full()
        .h(30.0)
        .rounded(7.0)
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.08))
        .bg(Color::rgba(0.055, 0.062, 0.076, 1.0))
        .items_center()
        .justify_between()
        .p_px(8.0)
        .cursor_pointer()
        .child(
            text(label)
                .size(12.0)
                .color(Color::rgba(0.84, 0.88, 0.94, 1.0)),
        )
        .child(
            text(if is_open { "Hide" } else { "Show" })
                .size(12.0)
                .color(Color::rgba(0.95, 0.67, 0.33, 1.0)),
        );

    if let Some(state) = open_code_state {
        row = row.on_click(move |_| {
            let next = if is_open {
                String::new()
            } else {
                panel_id.to_string()
            };
            state.set_rebuild(next);
        });
    }

    row
}

fn interaction_demo_entries(family: ChartFamily) -> Vec<InteractionDemoEntry> {
    if supports_x_bindings(family) {
        let mut demos = vec![
            demo_entry(
                InteractionDemoKind::XPanAndHover,
                "Hover + pan + pinch zoom",
                "Move the pointer over this chart for hover state, drag horizontally to pan X, and pinch over the chart to zoom X. Vertical wheel keeps scrolling this page.",
                x_interaction_code(family),
                "The gallery keeps ChartInputBindings::scroll_zoom disabled so wheel gestures that start inside the chart still move the content page.",
            ),
            demo_entry(
                InteractionDemoKind::XShiftBrush,
                "Shift+drag X brush",
                "Hold Shift before pressing the mouse button, then drag across this chart to create an X-range brush.",
                "bindings.scroll_zoom = false; bindings.brush_drag.required = ModifiersReq::shift()",
                "The same selected chart model starts brush mode only when the Shift modifier is present on mouse-down.",
            ),
            demo_entry(
                InteractionDemoKind::XDragBrush,
                "Drag-only brush binding",
                "Drag without Shift. This version disables pan and maps plain drag directly to brush selection.",
                "bindings.scroll_zoom = false; bindings.pan_drag = DragBinding::none(); bindings.brush_drag.required = ModifiersReq::none()",
                "This is the selection-first variant for the selected chart family, useful when pan should not steal drag gestures.",
            ),
        ];

        if supports_linked_sync(family) {
            demos.push(demo_entry(
                InteractionDemoKind::XLinkedSync,
                "Linked domain sync",
                "Drag, pinch, hover, or Shift+drag either chart row; both rows use the selected family and share one ChartLinkHandle.",
                linked_code(family),
                "The linked builder for this family synchronizes x-domain, hover x, and brush selection across multiple chart instances.",
            ));
        }
        return demos;
    }

    match family {
        ChartFamily::Contour | ChartFamily::DensityMap => vec![demo_entry(
            InteractionDemoKind::TwoDimensional,
            "2D pan + pinch zoom + rectangle brush",
            "Drag to pan both axes, pinch over the chart to zoom, and Shift+drag to create a rectangular selection in data coordinates. Vertical wheel keeps scrolling this page.",
            "model.style.scroll_zoom_factor = 0.0; contour_chart(handle) / density_map_chart(handle)",
            "The selected surface chart uses 2D data-space transforms while leaving wheel scrolling to the surrounding gallery.",
        )],
        ChartFamily::Geo | ChartFamily::Network => vec![demo_entry(
            InteractionDemoKind::TwoDimensional,
            "2D navigation + hover inspection",
            "Drag to pan the projected space, pinch over the chart to zoom, and move the pointer to inspect the active coordinate or node. Vertical wheel keeps scrolling this page.",
            "model.style.scroll_zoom_factor = 0.0; geo_chart(handle) / network_chart(handle)",
            "This selected chart family has graph/projection navigation, but not X-range brush semantics.",
        )],
        ChartFamily::Polar | ChartFamily::Hierarchy => vec![demo_entry(
            InteractionDemoKind::HoverOnly,
            "Hover-only inspection",
            "Move across regions or dimensions. There is no domain pan or brush because this chart is layout-driven.",
            hover_only_code(family),
            "Pointer movement updates hover state for the selected layout without changing the underlying data domain.",
        )],
        ChartFamily::Heatmap | ChartFamily::Gauge | ChartFamily::Funnel => vec![demo_entry(
            InteractionDemoKind::StaticModel,
            "Static/model-driven render",
            "Inspect the rendered model output. This family is updated by rebuilding host model state rather than pointer gestures.",
            static_code(family),
            "The selected chart demonstrates validation, scale/budget behavior, and model-driven rendering with no drag or zoom state.",
        )],
        _ => vec![demo_entry(
            InteractionDemoKind::StaticModel,
            "Static/model-driven render",
            "Inspect the rendered model output for this chart family.",
            "build_chart(family)",
            "This fallback keeps the selected chart visible without adding unsupported interaction semantics.",
        )],
    }
}

fn demo_entry(
    kind: InteractionDemoKind,
    title: &'static str,
    instruction: &'static str,
    code_change: &'static str,
    effect: &'static str,
) -> InteractionDemoEntry {
    InteractionDemoEntry {
        spec: InteractionDemo {
            title,
            instruction,
            code_change,
            effect,
        },
        kind,
    }
}

fn supports_x_bindings(family: ChartFamily) -> bool {
    matches!(
        family,
        ChartFamily::LinkedLine
            | ChartFamily::LinkedArea
            | ChartFamily::LinkedBar
            | ChartFamily::MultiLine
            | ChartFamily::StackedArea
            | ChartFamily::Scatter
            | ChartFamily::Candlestick
            | ChartFamily::Histogram
            | ChartFamily::Statistics
    )
}

fn supports_linked_sync(family: ChartFamily) -> bool {
    matches!(
        family,
        ChartFamily::LinkedLine
            | ChartFamily::LinkedArea
            | ChartFamily::LinkedBar
            | ChartFamily::MultiLine
            | ChartFamily::StackedArea
            | ChartFamily::Scatter
            | ChartFamily::Candlestick
    )
}

fn x_interaction_code(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::LinkedLine => {
            "bindings.scroll_zoom = false; line_chart_with_bindings(LineChartHandle::new(model), bindings)"
        }
        ChartFamily::LinkedArea => {
            "bindings.scroll_zoom = false; area_chart_with_bindings(AreaChartHandle::new(model), bindings)"
        }
        ChartFamily::LinkedBar => {
            "bindings.scroll_zoom = false; bar_chart_with_bindings(BarChartHandle::new(model), bindings)"
        }
        ChartFamily::MultiLine => {
            "bindings.scroll_zoom = false; multi_line_chart_with_bindings(MultiLineChartHandle::new(model), bindings)"
        }
        ChartFamily::StackedArea => {
            "bindings.scroll_zoom = false; stacked_area_chart_with_bindings(StackedAreaChartHandle::new(model), bindings)"
        }
        ChartFamily::Scatter => {
            "bindings.scroll_zoom = false; scatter_chart_with_bindings(ScatterChartHandle::new(model), bindings)"
        }
        ChartFamily::Candlestick => {
            "bindings.scroll_zoom = false; candlestick_chart_with_bindings(CandlestickChartHandle::new(model), bindings)"
        }
        ChartFamily::Histogram => {
            "bindings.scroll_zoom = false; histogram_chart_with_bindings(HistogramChartHandle::new(model), bindings)"
        }
        ChartFamily::Statistics => {
            "bindings.scroll_zoom = false; statistics_chart_with_bindings(StatisticsChartHandle::new(model), bindings)"
        }
        _ => "bindings.scroll_zoom = false; chart_with_bindings(handle, bindings)",
    }
}

fn linked_code(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::LinkedLine => {
            "linked_line_chart_with_bindings(handle, link.clone(), bindings)"
        }
        ChartFamily::LinkedArea => {
            "linked_area_chart_with_bindings(handle, link.clone(), bindings)"
        }
        ChartFamily::LinkedBar => "linked_bar_chart_with_bindings(handle, link.clone(), bindings)",
        ChartFamily::MultiLine => {
            "linked_multi_line_chart_with_bindings(handle, link.clone(), bindings)"
        }
        ChartFamily::StackedArea => {
            "linked_stacked_area_chart_with_bindings(handle, link.clone(), bindings)"
        }
        ChartFamily::Scatter => {
            "linked_scatter_chart_with_bindings(handle, link.clone(), bindings)"
        }
        ChartFamily::Candlestick => {
            "linked_candlestick_chart_with_bindings(handle, link.clone(), bindings)"
        }
        _ => "linked_*_chart_with_bindings(handle, link.clone(), bindings)",
    }
}

fn hover_only_code(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::Polar => "polar_chart(PolarChartHandle::new(model))",
        ChartFamily::Hierarchy => "hierarchy_chart(HierarchyChartHandle::new(model))",
        _ => "hover_chart(handle)",
    }
}

fn static_code(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::Heatmap => "heatmap_chart(HeatmapChartHandle::new(model))",
        ChartFamily::Gauge => "gauge_chart(GaugeChartHandle::new(model))",
        ChartFamily::Funnel => "funnel_chart(FunnelChartHandle::new(model))",
        _ => "static_chart(handle)",
    }
}

fn interaction_demo_chart(family: ChartFamily, kind: InteractionDemoKind) -> anyhow::Result<Div> {
    match kind {
        InteractionDemoKind::XPanAndHover | InteractionDemoKind::XShiftBrush => {
            let bindings = page_chart_bindings();
            x_family_chart(family, bindings)
        }
        InteractionDemoKind::XDragBrush => x_family_chart(family, drag_only_brush_bindings()),
        InteractionDemoKind::XLinkedSync => linked_family_chart(family),
        InteractionDemoKind::TwoDimensional => two_dimensional_family_chart(family),
        InteractionDemoKind::HoverOnly => hover_only_family_chart(family),
        InteractionDemoKind::StaticModel => static_family_chart(family),
    }
}

fn drag_only_brush_bindings() -> ChartInputBindings {
    ChartInputBindings {
        brush_drag: DragBinding {
            required: ModifiersReq::none(),
            action: DragAction::BrushX,
        },
        pan_drag: DragBinding::none(),
        scroll_zoom: false,
    }
}

fn x_family_chart(family: ChartFamily, bindings: ChartInputBindings) -> anyhow::Result<Div> {
    Ok(match family {
        ChartFamily::LinkedLine => {
            let linked = linked_series()?;
            let mut model = LineChartModel::new(linked.line);
            model.style.line = Color::rgba(0.10, 0.72, 0.84, 1.0);
            model.style.stroke_width = 2.0;
            chart_surface(line_chart_with_bindings(
                LineChartHandle::new(model),
                bindings,
            ))
        }
        ChartFamily::LinkedArea => {
            let linked = linked_series()?;
            let mut model = AreaChartModel::new(linked.area);
            model.style.line = Color::rgba(0.95, 0.65, 0.20, 1.0);
            model.style.area = Color::rgba(0.95, 0.65, 0.20, 0.25);
            chart_surface(area_chart_with_bindings(
                AreaChartHandle::new(model),
                bindings,
            ))
        }
        ChartFamily::LinkedBar => {
            let linked = linked_series()?;
            let mut model = BarChartModel::new(linked.bar).context("bar interaction")?;
            model.style.stacked = false;
            model.style.bar_alpha = 0.70;
            chart_surface(bar_chart_with_bindings(
                BarChartHandle::new(model),
                bindings,
            ))
        }
        ChartFamily::MultiLine => {
            let mut model = MultiLineChartModel::new(multi_line_series()?)?;
            model.set_gap_dx(9.0);
            chart_surface(multi_line_chart_with_bindings(
                MultiLineChartHandle::new(model),
                bindings,
            ))
        }
        ChartFamily::StackedArea => {
            let mut model = StackedAreaChartModel::new(stacked_area_series()?)?;
            model.style.mode = StackedAreaMode::Streamgraph;
            chart_surface(stacked_area_chart_with_bindings(
                StackedAreaChartHandle::new(model),
                bindings,
            ))
        }
        ChartFamily::Scatter => {
            let mut model = ScatterChartModel::new(scatter_series()?);
            model.set_max_points(1_200);
            model.style.hover_hit_radius_px = 18.0;
            chart_surface(scatter_chart_with_bindings(
                ScatterChartHandle::new(model),
                bindings,
            ))
        }
        ChartFamily::Candlestick => chart_surface(candlestick_chart_with_bindings(
            CandlestickChartHandle::new(CandlestickChartModel::new(candle_series()?)),
            bindings,
        )),
        ChartFamily::Histogram => chart_surface(histogram_chart_with_bindings(
            HistogramChartHandle::new(
                HistogramChartModel::new(histogram_values()).context("histogram interaction")?,
            ),
            bindings,
        )),
        ChartFamily::Statistics => {
            let mut model =
                StatisticsChartModel::new(statistics_groups()).context("statistics interaction")?;
            model.style.mode = StatisticsMode::Violin;
            chart_surface(statistics_chart_with_bindings(
                StatisticsChartHandle::new(model),
                bindings,
            ))
        }
        _ => build_chart(family)?,
    })
}

fn linked_family_chart(family: ChartFamily) -> anyhow::Result<Div> {
    let link = chart_link(0.0, (SERIES_POINTS - 1) as f32);
    let bindings = page_chart_bindings();
    let mut panel = div().w_full().h_full().flex_col().gap_px(8.0);

    match family {
        ChartFamily::LinkedLine => {
            for color in [
                Color::rgba(0.10, 0.72, 0.84, 1.0),
                Color::rgba(0.95, 0.65, 0.20, 1.0),
            ] {
                let linked = linked_series()?;
                let mut model = LineChartModel::new(linked.line);
                model.style.line = color;
                model.style.stroke_width = 2.0;
                panel = panel.child(linked_row(linked_line_chart_with_bindings(
                    LineChartHandle::new(model),
                    link.clone(),
                    bindings,
                )));
            }
        }
        ChartFamily::LinkedArea => {
            for alpha in [0.25, 0.38] {
                let linked = linked_series()?;
                let mut model = AreaChartModel::new(linked.area);
                model.style.line = Color::rgba(0.95, 0.65, 0.20, 1.0);
                model.style.area = Color::rgba(0.95, 0.65, 0.20, alpha);
                panel = panel.child(linked_row(linked_area_chart_with_bindings(
                    AreaChartHandle::new(model),
                    link.clone(),
                    bindings,
                )));
            }
        }
        ChartFamily::LinkedBar => {
            for stacked in [false, true] {
                let linked = linked_series()?;
                let mut model = BarChartModel::new(linked.bar).context("linked bar demo")?;
                model.style.stacked = stacked;
                model.style.bar_alpha = 0.70;
                panel = panel.child(linked_row(linked_bar_chart_with_bindings(
                    BarChartHandle::new(model),
                    link.clone(),
                    bindings,
                )));
            }
        }
        ChartFamily::MultiLine => {
            for gap in [9.0, 14.0] {
                let mut model = MultiLineChartModel::new(multi_line_series()?)?;
                model.set_gap_dx(gap);
                panel = panel.child(linked_row(linked_multi_line_chart_with_bindings(
                    MultiLineChartHandle::new(model),
                    link.clone(),
                    bindings,
                )));
            }
        }
        ChartFamily::StackedArea => {
            for mode in [StackedAreaMode::Stacked, StackedAreaMode::Streamgraph] {
                let mut model = StackedAreaChartModel::new(stacked_area_series()?)?;
                model.style.mode = mode;
                panel = panel.child(linked_row(linked_stacked_area_chart_with_bindings(
                    StackedAreaChartHandle::new(model),
                    link.clone(),
                    bindings,
                )));
            }
        }
        ChartFamily::Scatter => {
            for max_points in [900, 1_200] {
                let mut model = ScatterChartModel::new(scatter_series()?);
                model.set_max_points(max_points);
                panel = panel.child(linked_row(linked_scatter_chart_with_bindings(
                    ScatterChartHandle::new(model),
                    link.clone(),
                    bindings,
                )));
            }
        }
        ChartFamily::Candlestick => {
            for stroke_width in [1.0, 1.5] {
                let mut model = CandlestickChartModel::new(candle_series()?);
                model.style.stroke_width = stroke_width;
                panel = panel.child(linked_row(linked_candlestick_chart_with_bindings(
                    CandlestickChartHandle::new(model),
                    link.clone(),
                    bindings,
                )));
            }
        }
        _ => return x_family_chart(family, bindings),
    }

    Ok(panel)
}

fn linked_row(chart: impl ElementBuilder + 'static) -> Div {
    div().w_full().h(112.0).child(chart)
}

fn two_dimensional_family_chart(family: ChartFamily) -> anyhow::Result<Div> {
    match family {
        ChartFamily::Contour => {
            let mut model = ContourChartModel::new(SURFACE_W, SURFACE_H, surface_values())
                .context("contour demo")?;
            model.style.levels = vec![-18.0, -6.0, 6.0, 18.0, 30.0];
            model.style.scroll_zoom_factor = PAGE_SCROLL_ZOOM_FACTOR;
            Ok(chart_surface(contour_chart(ContourChartHandle::new(model))))
        }
        ChartFamily::DensityMap => {
            let mut model = DensityMapChartModel::new(density_points()).context("density demo")?;
            model.style.scroll_zoom_factor = PAGE_SCROLL_ZOOM_FACTOR;
            Ok(chart_surface(density_map_chart(
                DensityMapChartHandle::new(model),
            )))
        }
        ChartFamily::Geo => {
            let mut model = GeoChartModel::new(geo_shapes()).context("geo demo")?;
            model.style.scroll_zoom_factor = PAGE_SCROLL_ZOOM_FACTOR;
            Ok(chart_surface(geo_chart(GeoChartHandle::new(model))))
        }
        ChartFamily::Network => {
            let mut model = NetworkChartModel::new_sankey(network_nodes(), network_links())
                .context("network demo")?;
            model.style.scroll_zoom_factor = PAGE_SCROLL_ZOOM_FACTOR;
            Ok(chart_surface(network_chart(NetworkChartHandle::new(model))))
        }
        _ => build_chart(family),
    }
}

fn hover_only_family_chart(family: ChartFamily) -> anyhow::Result<Div> {
    match family {
        ChartFamily::Polar => {
            let mut model = PolarChartModel::new_radar(polar_dimensions(), polar_series())
                .context("polar demo")?;
            model.mode = PolarChartMode::Radar;
            Ok(chart_surface(polar_chart(PolarChartHandle::new(model))))
        }
        ChartFamily::Hierarchy => {
            let mut model = HierarchyChartModel::new(hierarchy_root()).context("hierarchy demo")?;
            model.style.mode = HierarchyMode::Treemap;
            Ok(chart_surface(hierarchy_chart(HierarchyChartHandle::new(
                model,
            ))))
        }
        _ => build_chart(family),
    }
}

fn static_family_chart(family: ChartFamily) -> anyhow::Result<Div> {
    match family {
        ChartFamily::Heatmap => Ok(chart_surface(heatmap_chart(HeatmapChartHandle::new(
            HeatmapChartModel::new(HEATMAP_W, HEATMAP_H, heatmap_values())
                .context("heatmap demo")?,
        )))),
        ChartFamily::Gauge => Ok(chart_surface(gauge_chart(GaugeChartHandle::new(
            GaugeChartModel::new(0.0, 100.0, 72.0).context("gauge demo")?,
        )))),
        ChartFamily::Funnel => Ok(chart_surface(funnel_chart(FunnelChartHandle::new(
            FunnelChartModel::new(funnel_stages()).context("funnel demo")?,
        )))),
        _ => build_chart(family),
    }
}

fn error_view(width: f32, height: f32, error: anyhow::Error) -> Div {
    div()
        .w(width)
        .h(height)
        .bg(Color::rgba(0.06, 0.07, 0.09, 1.0))
        .items_center()
        .justify_center()
        .child(
            text(format!(
                "blinc_charts_desktop failed to build gallery: {error}"
            ))
            .size(16.0)
            .color(Color::rgba(0.95, 0.78, 0.62, 1.0)),
        )
}

fn example(family: ChartFamily) -> Option<&'static GalleryExample> {
    EXAMPLES.iter().find(|example| example.family == family)
}

fn group_label(label: &'static str) -> Div {
    div().w_full().h_fit().child(
        text(label)
            .size(11.0)
            .color(Color::rgba(0.54, 0.60, 0.67, 1.0)),
    )
}

fn metric_chip(label: &str) -> Div {
    div()
        .w(78.0)
        .h(22.0)
        .rounded(4.0)
        .items_center()
        .justify_center()
        .bg(Color::rgba(0.10, 0.11, 0.13, 0.55))
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.04))
        .child(
            text(label.to_string())
                .size(11.0)
                .color(Color::rgba(0.68, 0.74, 0.82, 1.0)),
        )
}

fn chips(labels: &[&'static str]) -> Div {
    let mut row = div().w_full().h_fit().flex_row().gap_px(6.0);
    for label in labels.iter().take(6) {
        row = row.child(
            div()
                .h(20.0)
                .w((label.len() as f32 * 6.2 + 12.0).clamp(52.0, 156.0))
                .rounded(4.0)
                .items_center()
                .justify_center()
                .bg(Color::rgba(0.095, 0.105, 0.125, 0.42))
                .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.035))
                .child(
                    text(*label)
                        .size(10.0)
                        .color(Color::rgba(0.58, 0.65, 0.74, 1.0)),
                ),
        );
    }
    row
}

fn info_grid(items: &[(&'static str, &'static str)]) -> Div {
    let mut grid = div().w_full().h_fit().flex_row().gap_px(10.0);
    for (label, body) in items {
        grid = grid.child(
            div()
                .flex_1()
                .h(92.0)
                .rounded(8.0)
                .bg(Color::rgba(0.070, 0.078, 0.094, 1.0))
                .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.08))
                .p_px(10.0)
                .flex_col()
                .gap_px(6.0)
                .child(
                    text(*label)
                        .size(11.0)
                        .color(Color::rgba(0.95, 0.67, 0.33, 1.0)),
                )
                .child(
                    text(*body)
                        .size(12.0)
                        .color(Color::rgba(0.76, 0.81, 0.86, 1.0)),
                ),
        );
    }
    grid
}

fn section_heading(label: &'static str) -> Div {
    div().w_full().h_fit().child(
        text(label)
            .size(16.0)
            .color(Color::rgba(0.90, 0.94, 0.98, 1.0)),
    )
}

fn variant_card(name: &'static str, code_change: &'static str, effect: &'static str) -> Div {
    div()
        .w_full()
        .h(94.0)
        .rounded(8.0)
        .bg(Color::rgba(0.070, 0.078, 0.094, 1.0))
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.08))
        .p_px(10.0)
        .flex_col()
        .gap_px(6.0)
        .child(
            text(name)
                .size(14.0)
                .color(Color::rgba(0.94, 0.97, 1.0, 1.0)),
        )
        .child(
            text(code_change)
                .size(12.0)
                .color(Color::rgba(0.95, 0.67, 0.33, 1.0)),
        )
        .child(
            text(effect)
                .size(12.0)
                .color(Color::rgba(0.74, 0.79, 0.85, 1.0)),
        )
}

fn body_block(title: &'static str, body: &'static [&'static str]) -> Div {
    let mut block = div()
        .w_full()
        .h(86.0)
        .rounded(8.0)
        .bg(Color::rgba(0.070, 0.078, 0.094, 1.0))
        .border(1.0, Color::rgba(1.0, 1.0, 1.0, 0.08))
        .p_px(12.0)
        .flex_col()
        .gap_px(7.0)
        .child(
            div().w_full().h(18.0).child(
                text(title)
                    .size(14.0)
                    .color(Color::rgba(0.94, 0.97, 1.0, 1.0)),
            ),
        );

    for line in body {
        block = block.child(
            div().w_full().h(16.0).child(
                text(*line)
                    .size(12.0)
                    .color(Color::rgba(0.74, 0.79, 0.85, 1.0)),
            ),
        );
    }

    block
}

impl GalleryGroup {
    fn label(self) -> &'static str {
        match self {
            GalleryGroup::Linked => "Linked pan / zoom",
            GalleryGroup::TimeSeries => "Time series",
            GalleryGroup::Distribution => "Distribution",
            GalleryGroup::Surface => "2D / surface",
            GalleryGroup::Structure => "Structure",
            GalleryGroup::Indicator => "Indicator / radial",
        }
    }
}

impl GalleryTab {
    fn label(self) -> &'static str {
        match self {
            GalleryTab::Example => "Example",
            GalleryTab::Code => "Code",
            GalleryTab::Variants => "Variants",
            GalleryTab::Interactions => "Interactions",
            GalleryTab::Explanation => "Guide",
        }
    }
}

fn data_shape(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::LinkedLine | ChartFamily::LinkedArea | ChartFamily::Scatter => {
            "TimeSeriesF32: sorted X values with one Y value per X."
        }
        ChartFamily::LinkedBar | ChartFamily::MultiLine | ChartFamily::StackedArea => {
            "Vec<TimeSeriesF32>: multiple aligned or partially aligned series."
        }
        ChartFamily::Candlestick => "CandleSeries: sorted OHLC candles.",
        ChartFamily::Histogram => "Vec<f32>: one numeric sample per observation.",
        ChartFamily::Statistics => "Vec<Vec<f32>>: grouped numeric samples.",
        ChartFamily::Heatmap | ChartFamily::Contour => "Row-major rectangular grid values.",
        ChartFamily::DensityMap => "Vec<Point>: many XY points in data space.",
        ChartFamily::Gauge => "A single clamped value inside a min/max range.",
        ChartFamily::Funnel => "Ordered stage labels and positive values.",
        ChartFamily::Polar => "Dimensions plus same-length series vectors.",
        ChartFamily::Geo => "Vec<Vec<Point>>: multiple shape outlines.",
        ChartFamily::Hierarchy => "HierarchyNode tree with finite leaf weights.",
        ChartFamily::Network => "Node labels plus graph, Sankey, or chord edges.",
    }
}

fn interaction_hint(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::Contour
        | ChartFamily::DensityMap
        | ChartFamily::Geo
        | ChartFamily::Network => {
            "Use pinch to zoom, drag to pan, and hover to inspect the current 2D position."
        }
        ChartFamily::Gauge | ChartFamily::Funnel | ChartFamily::Heatmap => {
            "Inspect the static output and compare the variant notes for style and budget changes."
        }
        ChartFamily::Hierarchy | ChartFamily::Polar => {
            "Hover regions or dimensions; variants change the layout interpretation."
        }
        _ => "Use pinch to zoom X, drag to pan, and Shift+drag to create a brush selection.",
    }
}

fn budget_note(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::LinkedLine | ChartFamily::MultiLine | ChartFamily::Scatter => {
            "Sampling and LOD keep point-heavy views under renderer budgets."
        }
        ChartFamily::Heatmap | ChartFamily::DensityMap | ChartFamily::Contour => {
            "Cell and segment caps trade visual resolution for predictable frame cost."
        }
        ChartFamily::Network | ChartFamily::Hierarchy | ChartFamily::Geo => {
            "Node, leaf, link, and point caps avoid overloading the GPU primitive budget."
        }
        _ => "Constructor validation keeps invalid data from reaching render paths.",
    }
}

fn constructor_note(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::LinkedLine => "LineChartModel::new(series) accepts one TimeSeriesF32.",
        ChartFamily::LinkedArea => "AreaChartModel::new(series) accepts one TimeSeriesF32.",
        ChartFamily::LinkedBar => "BarChartModel::new(series_vec) validates one or more series.",
        ChartFamily::MultiLine => "MultiLineChartModel::new(series_vec) requires non-empty series.",
        ChartFamily::StackedArea => {
            "StackedAreaChartModel::new(series_vec) supports misaligned X samples."
        }
        ChartFamily::Scatter => {
            "ScatterChartModel::new(series) builds a point cloud from TimeSeriesF32."
        }
        ChartFamily::Candlestick => "CandleSeries::new(candles) validates sorted OHLC input.",
        ChartFamily::Histogram => "HistogramChartModel::new(values) ignores non-finite samples.",
        ChartFamily::Statistics => {
            "StatisticsChartModel::new(groups) requires finite grouped values."
        }
        ChartFamily::Heatmap => "HeatmapChartModel::new(w, h, values) requires exact grid length.",
        ChartFamily::Contour => {
            "ContourChartModel::new(w, h, values) uses row-major scalar fields."
        }
        ChartFamily::DensityMap => {
            "DensityMapChartModel::new(points) bins visible-domain XY points."
        }
        ChartFamily::Gauge => "GaugeChartModel::new(min, max, value) clamps value to range.",
        ChartFamily::Funnel => "FunnelChartModel::new(stages) normalizes positive stage values.",
        ChartFamily::Polar => {
            "PolarChartModel::new_radar(dimensions, series) validates dimensions."
        }
        ChartFamily::Geo => "GeoChartModel::new(shapes) needs at least one finite point.",
        ChartFamily::Hierarchy => {
            "HierarchyChartModel::new(root) validates labels and positive weights."
        }
        ChartFamily::Network => "NetworkChartModel::new_* validates node/link/matrix shape.",
    }
}

fn code_snippet(family: ChartFamily) -> &'static str {
    match family {
        ChartFamily::LinkedLine => {
            r#"let link = chart_link(0.0, 159.0);
let model = LineChartModel::new(series);
let handle = LineChartHandle::new(model);
linked_line_chart(handle, link)"#
        }
        ChartFamily::LinkedArea => {
            r#"let mut model = AreaChartModel::new(series);
model.style.baseline_y = 0.0;
let handle = AreaChartHandle::new(model);
linked_area_chart(handle, link)"#
        }
        ChartFamily::LinkedBar => {
            r#"let mut model = BarChartModel::new(vec![north, south])?;
model.style.stacked = false;
let handle = BarChartHandle::new(model);
linked_bar_chart(handle, link)"#
        }
        ChartFamily::MultiLine => {
            r#"let mut model = MultiLineChartModel::new(vec![north, south])?;
model.set_gap_dx(9.0);
model.style.max_points_per_series = 12_000;
multi_line_chart(MultiLineChartHandle::new(model))"#
        }
        ChartFamily::StackedArea => {
            r#"let mut model = StackedAreaChartModel::new(vec![north, south])?;
model.style.mode = StackedAreaMode::Streamgraph;
stacked_area_chart(StackedAreaChartHandle::new(model))"#
        }
        ChartFamily::Scatter => {
            r#"let mut model = ScatterChartModel::new(series);
model.set_max_points(1_200);
model.style.hover_hit_radius_px = 14.0;
scatter_chart(ScatterChartHandle::new(model))"#
        }
        ChartFamily::Candlestick => {
            r#"let series = CandleSeries::new(candles)?;
let model = CandlestickChartModel::new(series);
candlestick_chart(CandlestickChartHandle::new(model))"#
        }
        ChartFamily::Histogram => {
            r#"let mut model = HistogramChartModel::new(values)?;
model.style.bins = 48;
histogram_chart(HistogramChartHandle::new(model))"#
        }
        ChartFamily::Statistics => {
            r#"let mut model = StatisticsChartModel::new(groups)?;
model.style.mode = StatisticsMode::Violin;
statistics_chart(StatisticsChartHandle::new(model))"#
        }
        ChartFamily::Heatmap => {
            r#"let model = HeatmapChartModel::new(grid_w, grid_h, values)?;
heatmap_chart(HeatmapChartHandle::new(model))"#
        }
        ChartFamily::Contour => {
            r#"let mut model = ContourChartModel::new(grid_w, grid_h, values)?;
model.style.levels = vec![-18.0, -6.0, 6.0, 18.0];
contour_chart(ContourChartHandle::new(model))"#
        }
        ChartFamily::DensityMap => {
            r#"let mut model = DensityMapChartModel::new(points)?;
model.style.max_cells_x = 128;
density_map_chart(DensityMapChartHandle::new(model))"#
        }
        ChartFamily::Gauge => {
            r#"let mut model = GaugeChartModel::new(0.0, 100.0, 72.0)?;
model.set_value_transition(86.0, 0.45);
gauge_chart(GaugeChartHandle::new(model))"#
        }
        ChartFamily::Funnel => {
            r#"let stages = vec![("Visitors".into(), 12000.0), ("Paid".into(), 1480.0)];
let model = FunnelChartModel::new(stages)?;
funnel_chart(FunnelChartHandle::new(model))"#
        }
        ChartFamily::Polar => {
            r#"let rows = vec![vec![0.8, 0.6, 0.4, 0.3], vec![0.5, 0.9, 0.7, 0.6]];
let mut model = PolarChartModel::new_radar(dimensions, rows)?;
model.mode = PolarChartMode::Radar;
polar_chart(PolarChartHandle::new(model))"#
        }
        ChartFamily::Geo => {
            r#"let mut model = GeoChartModel::new(shapes)?;
model.style.max_points = 20_000;
geo_chart(GeoChartHandle::new(model))"#
        }
        ChartFamily::Hierarchy => {
            r#"let mut model = HierarchyChartModel::new(root)?;
model.style.mode = HierarchyMode::Treemap;
hierarchy_chart(HierarchyChartHandle::new(model))"#
        }
        ChartFamily::Network => {
            r#"let model = NetworkChartModel::new_sankey(nodes, links)?;
network_chart(NetworkChartHandle::new(model))"#
        }
    }
}

fn variant_notes(family: ChartFamily) -> &'static [(&'static str, &'static str, &'static str)] {
    match family {
        ChartFamily::LinkedBar => &[
            (
                "Grouped bars",
                "model.style.stacked = false",
                "Compares each series side-by-side at the same X value.",
            ),
            (
                "Stacked bars",
                "model.style.stacked = true",
                "Emphasizes total contribution while preserving series color.",
            ),
        ],
        ChartFamily::StackedArea => &[
            (
                "Stacked",
                "model.style.mode = StackedAreaMode::Stacked",
                "Uses zero as baseline, best for absolute part-to-whole totals.",
            ),
            (
                "Streamgraph",
                "model.style.mode = StackedAreaMode::Streamgraph",
                "Centers the layers to highlight flow and relative movement.",
            ),
        ],
        ChartFamily::Statistics => &[
            (
                "Boxplot",
                "model.style.mode = StatisticsMode::Boxplot",
                "Compact median, quartile, and whisker summary.",
            ),
            (
                "Violin",
                "model.style.mode = StatisticsMode::Violin",
                "Shows distribution shape by mirroring estimated density.",
            ),
            (
                "Error band",
                "model.style.mode = StatisticsMode::ErrorBand",
                "Shows central tendency with uncertainty-like ranges.",
            ),
        ],
        ChartFamily::Polar => &[
            (
                "Radar",
                "model.mode = PolarChartMode::Radar",
                "Compares multivariate profiles around a shared center.",
            ),
            (
                "Polar",
                "model.mode = PolarChartMode::Polar",
                "Interprets values on radial axes with angular dimensions.",
            ),
            (
                "Parallel",
                "model.mode = PolarChartMode::Parallel",
                "Uses parallel coordinates for clearer dimension-by-dimension comparison.",
            ),
        ],
        ChartFamily::Hierarchy => &[
            (
                "Treemap",
                "model.style.mode = HierarchyMode::Treemap",
                "Optimizes screen usage for leaf weight comparison.",
            ),
            (
                "Sunburst",
                "model.style.mode = HierarchyMode::Sunburst",
                "Makes parent-child depth visually explicit in radial bands.",
            ),
            (
                "Packing",
                "model.style.mode = HierarchyMode::Packing",
                "Shows clusters as nested circle-like regions.",
            ),
        ],
        ChartFamily::Network => &[
            (
                "Graph",
                "NetworkChartModel::new_graph(nodes, edges)?",
                "Shows topology and supports graph pan/zoom.",
            ),
            (
                "Sankey",
                "NetworkChartModel::new_sankey(nodes, links)?",
                "Shows weighted flow from sources to sinks.",
            ),
            (
                "Chord",
                "NetworkChartModel::new_chord(labels, matrix)?",
                "Shows dense pairwise relationships from an NxN matrix.",
            ),
        ],
        ChartFamily::Heatmap | ChartFamily::DensityMap => &[
            (
                "Higher resolution",
                "model.style.max_cells_x = 128; model.style.max_cells_y = 96",
                "Reveals more detail while increasing draw cost.",
            ),
            (
                "Lower resolution",
                "model.style.max_cells_x = 32; model.style.max_cells_y = 24",
                "Keeps frame cost predictable for large fields.",
            ),
        ],
        ChartFamily::Contour => &[
            (
                "Higher resolution",
                "model.style.max_segments = 4_000",
                "Reveals more contour detail while increasing draw cost.",
            ),
            (
                "Lower resolution",
                "model.style.max_segments = 250",
                "Keeps contour tessellation cost predictable for large fields.",
            ),
        ],
        ChartFamily::Gauge => &[
            (
                "Immediate value",
                "model.set_value(value)",
                "Updates the gauge state without animated interpolation.",
            ),
            (
                "Transitioned value",
                "model.set_value_transition(value, seconds)",
                "Animates toward the target using deterministic transition state.",
            ),
        ],
        ChartFamily::Geo => &[
            (
                "Interaction speed",
                "model.style.scroll_zoom_factor = 0.0",
                "Leaves wheel deltas for page scroll in this gallery.",
            ),
            (
                "Pinch floor",
                "model.style.pinch_zoom_min = 0.01",
                "Prevents invalid or too-small zoom factors during pinch gestures.",
            ),
            (
                "Budget cap",
                "model.style.max_points = 20_000",
                "Caps projected geometry detail for stable renderer cost.",
            ),
        ],
        ChartFamily::Funnel => &[
            (
                "Stage values",
                "let model = FunnelChartModel::new(stages)?",
                "Compares ordered funnel stages using positive stage values.",
            ),
            (
                "Budget cap",
                "stages.truncate(4)",
                "Keeps the rendered funnel compact by limiting stage count.",
            ),
        ],
        _ => &[
            (
                "Interaction speed",
                "style.scroll_zoom_factor = 0.01..0.04",
                "Controls how aggressively opt-in wheel deltas zoom the chart domain.",
            ),
            (
                "Pinch floor",
                "style.pinch_zoom_min = 0.01",
                "Prevents invalid or too-small zoom factors during pinch gestures.",
            ),
            (
                "Budget cap",
                "set_*_max_points or style.max_*",
                "Trades detail for stable renderer cost on large datasets.",
            ),
        ],
    }
}

fn interaction_notes(family: ChartFamily) -> &'static [(&'static str, &'static str, &'static str)] {
    match family {
        ChartFamily::LinkedLine | ChartFamily::LinkedArea | ChartFamily::LinkedBar => &[
            (
                "Shared linked state",
                "let link = chart_link(x0, x1); linked_*_chart(handle, link)",
                "Pan, zoom, hover X, and brush selection are shared across linked charts.",
            ),
            (
                "Custom bindings",
                "linked_*_chart_with_bindings(handle, link, bindings)",
                "Keeps the shared domain while changing which drag gesture pans or brushes.",
            ),
            (
                "Disable drag pan",
                "bindings.pan_drag = DragBinding::none()",
                "Plain drag stops moving X; pinch, hover, and brush remain active.",
            ),
        ],
        ChartFamily::MultiLine
        | ChartFamily::StackedArea
        | ChartFamily::Scatter
        | ChartFamily::Candlestick
        | ChartFamily::Histogram
        | ChartFamily::Statistics => &[
            (
                "Default gestures",
                "*_chart_with_bindings(handle, ChartInputBindings::default())",
                "Pinch zooms X; plain drag pans; Shift+drag creates a brush selection.",
            ),
            (
                "Brush-only drag",
                "bindings.pan_drag = DragBinding::none()",
                "Disables accidental panning while preserving Shift+drag selection.",
            ),
            (
                "Alt brush",
                "bindings.brush_drag.required = ModifiersReq { alt: true, ..ModifiersReq::none() }",
                "Moves selection from Shift+drag to Option/Alt+drag when Shift is reserved.",
            ),
        ],
        ChartFamily::Contour | ChartFamily::DensityMap => &[
            (
                "2D zoom speed",
                "model.style.scroll_zoom_factor = 0.0 in this gallery",
                "Disables wheel capture while pinch still zooms around the cursor.",
            ),
            (
                "2D rectangle brush",
                "Shift+drag inside the plot",
                "Creates a data-space rectangle and reports whether hover is inside it.",
            ),
            (
                "Pan inspection",
                "plain drag + hover",
                "Moves the 2D domain while hover text follows the current data coordinate.",
            ),
        ],
        ChartFamily::Geo | ChartFamily::Network => &[
            (
                "Projected pan",
                "plain drag",
                "Moves the projected coordinate space without changing source data.",
            ),
            (
                "Cursor zoom",
                "model.style.scroll_zoom_factor = 0.0 in this gallery",
                "Leaves wheel for page scroll while pinch preserves the inspected location under the cursor.",
            ),
            (
                "Hover budget",
                "style.max_* and hover radius",
                "Caps visible primitives while keeping hover lookup predictable.",
            ),
        ],
        ChartFamily::Polar | ChartFamily::Hierarchy => &[
            (
                "Hover target",
                "on_mouse_move updates hover state",
                "Highlights the active dimension, leaf, or region without changing layout mode.",
            ),
            (
                "Mode plus hover",
                "model.mode/style.mode = ...",
                "The same pointer movement explains different shapes after layout changes.",
            ),
            (
                "No drag domain",
                "no ChartInputBindings",
                "Uses hover-driven inspection rather than pan or brush domain editing.",
            ),
        ],
        ChartFamily::Gauge | ChartFamily::Funnel | ChartFamily::Heatmap => &[
            (
                "Static inspection",
                "no pointer binding",
                "The example renders model state; pointer gestures do not mutate domains.",
            ),
            (
                "Host-driven update",
                "model.set_value(...) or rebuild with new data",
                "External app state changes the indicator or grid before the next render.",
            ),
            (
                "Budget variant",
                "style.max_* or grid dimensions",
                "Changes rendered detail without adding pointer interaction state.",
            ),
        ],
    }
}

fn explanation_notes(family: ChartFamily) -> &'static [(&'static str, &'static [&'static str])] {
    match family {
        ChartFamily::LinkedLine | ChartFamily::LinkedArea | ChartFamily::LinkedBar => &[
            ("Linking", &["The linked builders share ChartLinkHandle state.", "X-domain, hover X, and brush selection can be synchronized across charts."]),
            ("Input model", &["Pinch zooms around the cursor position.", "Plain drag pans; Shift+drag creates a brush range."]),
            ("Data safety", &["Domains reject non-finite or inverted ranges.", "Selection endpoints are sorted and non-finite values are dropped."]),
        ],
        ChartFamily::MultiLine => &[
            ("Many series", &["Each series can be downsampled independently.", "Gap splitting prevents large missing intervals from being drawn as continuous lines."]),
            ("Density fallback", &["When series or segment budgets are exceeded, the chart can summarize density instead of drawing every segment."]),
        ],
        ChartFamily::StackedArea => &[
            ("Stacking", &["Values are aligned by X and accumulated into bands.", "Negative values and misaligned X samples are handled by model logic."]),
            ("Modes", &["Stacked mode preserves absolute totals.", "Streamgraph mode shifts the baseline for visual flow comparison."]),
        ],
        ChartFamily::Scatter => &[
            ("Hover", &["A spatial index accelerates nearest-point lookup for larger samples.", "The hit radius controls how forgiving hover selection feels."]),
            ("Budgets", &["Point caps limit GPU primitives.", "Small samples can use mesh triangulation paths internally."]),
        ],
        ChartFamily::Candlestick => &[
            ("OHLC", &["Each candle contains x, open, high, low, and close.", "The model expects sorted non-empty candles."]),
            ("Binning", &["Visible-domain binning avoids drawing too many candles at narrow pixel widths."]),
        ],
        ChartFamily::Histogram | ChartFamily::Statistics => &[
            ("Distribution", &["The chart explains shape rather than individual observations.", "Non-finite data is ignored or rejected before rendering."]),
            ("Variants", &["Changing bins or mode changes the statistical question being emphasized."]),
        ],
        ChartFamily::Heatmap | ChartFamily::Contour | ChartFamily::DensityMap => &[
            ("Surface", &["Values are mapped to cells, contour segments, or density bins.", "Budget fields bound how many primitives are emitted."]),
            ("2D navigation", &["Contour and density map support 2D pan, zoom, hover, and rectangle brush."]),
        ],
        ChartFamily::Gauge | ChartFamily::Funnel => &[
            ("Indicator", &["These charts summarize state, progress, or conversion rather than a continuous domain."]),
            ("Validation", &["Gauge values clamp to min/max.", "Funnel widths normalize against the largest positive stage."]),
        ],
        ChartFamily::Polar => &[
            ("Dimensions", &["Each series must have the same length as the dimension list.", "Hover selects the active dimension."]),
            ("Modes", &["Radar emphasizes profile shape.", "Parallel coordinates make per-dimension comparison easier."]),
        ],
        ChartFamily::Geo => &[
            ("Shapes", &["Each shape is a list of finite points.", "The chart computes a 2D domain from all finite points."]),
            ("Interaction", &["Pan and zoom move through the projected coordinate space."]),
        ],
        ChartFamily::Hierarchy => &[
            ("Tree layout", &["Internal nodes derive weight from children.", "Leaf labels and finite positive total weight are required."]),
            ("Mode choice", &["Treemap compares weight efficiently.", "Sunburst and icicle make depth easier to inspect."]),
        ],
        ChartFamily::Network => &[
            ("Relationship data", &["Graph uses unweighted topology.", "Sankey and chord use weights to express flow or pair strength."]),
            ("Validation", &["Node indices and matrix dimensions are checked before rendering."]),
        ],
    }
}

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

struct LinkedSeries {
    line: TimeSeriesF32,
    area: TimeSeriesF32,
    bar: Vec<TimeSeriesF32>,
}

fn multi_line_series() -> anyhow::Result<Vec<TimeSeriesF32>> {
    let x: Vec<f32> = (0..SERIES_POINTS).map(|idx| idx as f32).collect();
    (0..5)
        .map(|series| {
            let y = x
                .iter()
                .map(|x| {
                    40.0 + series as f32 * 9.0
                        + wave(*x, 0.07 + series as f32 * 0.013, 7.5)
                        + wave(*x, 0.19, 2.5)
                })
                .collect();
            TimeSeriesF32::new(x.clone(), y)
        })
        .collect()
}

fn stacked_area_series() -> anyhow::Result<Vec<TimeSeriesF32>> {
    let x: Vec<f32> = (0..SERIES_POINTS).map(|idx| idx as f32).collect();
    (0..4)
        .map(|series| {
            let y = x
                .iter()
                .map(|x| {
                    8.0 + ((*x * (0.05 + series as f32 * 0.018)).sin() + 1.2)
                        * (5.0 + series as f32 * 2.0)
                })
                .collect();
            TimeSeriesF32::new(x.clone(), y)
        })
        .collect()
}

fn scatter_series() -> anyhow::Result<TimeSeriesF32> {
    let x: Vec<f32> = (0..(SERIES_POINTS * 2))
        .map(|idx| idx as f32 * 0.5)
        .collect();
    let y = x
        .iter()
        .enumerate()
        .map(|(idx, x)| 36.0 + wave(*x, 0.18, 18.0) + ((idx % 23) as f32 - 11.0) * 0.7)
        .collect();
    TimeSeriesF32::new(x, y)
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

fn surface_values() -> Vec<f32> {
    let mut values = Vec::with_capacity(SURFACE_W * SURFACE_H);
    for y in 0..SURFACE_H {
        for x in 0..SURFACE_W {
            values.push(
                wave(x as f32, 0.32, 24.0)
                    + wave(y as f32, 0.45, 16.0)
                    + wave((x + y) as f32, 0.15, 10.0),
            );
        }
    }
    values
}

fn density_points() -> Vec<Point> {
    (0..900)
        .map(|idx| {
            let cluster = (idx % 3) as f32;
            let t = idx as f32;
            Point::new(
                cluster * 32.0 + wave(t, 0.17 + cluster * 0.02, 12.0) + 42.0,
                cluster * 18.0 + wave(t, 0.11 + cluster * 0.01, 10.0) + 34.0,
            )
        })
        .collect()
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

fn geo_shapes() -> Vec<Vec<Point>> {
    vec![
        vec![
            Point::new(0.0, 12.0),
            Point::new(18.0, 5.0),
            Point::new(34.0, 15.0),
            Point::new(30.0, 34.0),
            Point::new(9.0, 31.0),
            Point::new(0.0, 12.0),
        ],
        vec![
            Point::new(43.0, 9.0),
            Point::new(66.0, 8.0),
            Point::new(72.0, 24.0),
            Point::new(58.0, 39.0),
            Point::new(41.0, 30.0),
            Point::new(43.0, 9.0),
        ],
        vec![
            Point::new(20.0, 48.0),
            Point::new(39.0, 44.0),
            Point::new(55.0, 58.0),
            Point::new(46.0, 74.0),
            Point::new(22.0, 68.0),
            Point::new(20.0, 48.0),
        ],
    ]
}

fn hierarchy_root() -> HierarchyNode {
    HierarchyNode::node(
        "Portfolio",
        vec![
            HierarchyNode::node(
                "Acquire",
                vec![
                    HierarchyNode::leaf("Paid", 32.0),
                    HierarchyNode::leaf("Organic", 44.0),
                    HierarchyNode::leaf("Partner", 18.0),
                ],
            ),
            HierarchyNode::node(
                "Activate",
                vec![
                    HierarchyNode::leaf("Onboard", 28.0),
                    HierarchyNode::leaf("Habit", 22.0),
                    HierarchyNode::leaf("Invite", 13.0),
                ],
            ),
            HierarchyNode::node(
                "Retain",
                vec![
                    HierarchyNode::leaf("Core", 51.0),
                    HierarchyNode::leaf("Expansion", 31.0),
                    HierarchyNode::leaf("Risk", 11.0),
                ],
            ),
        ],
    )
}

fn network_nodes() -> Vec<String> {
    [
        "Traffic",
        "Signup",
        "Trial",
        "Active",
        "Paid",
        "Expansion",
        "Support",
        "Churn",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn network_links() -> Vec<(usize, usize, f32)> {
    vec![
        (0, 1, 90.0),
        (1, 2, 54.0),
        (2, 3, 41.0),
        (3, 4, 25.0),
        (4, 5, 12.0),
        (3, 6, 9.0),
        (6, 7, 4.0),
    ]
}

fn wave(x: f32, freq: f32, amp: f32) -> f32 {
    (x * freq).sin() * amp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_preview_allows_vertical_wheel_scroll() {
        assert!(!page_chart_bindings().scroll_zoom);
        assert_eq!(PAGE_SCROLL_ZOOM_FACTOR, 0.0);
    }

    #[test]
    fn interaction_demos_allow_vertical_wheel_scroll() {
        assert!(!page_chart_bindings().scroll_zoom);
    }
}
