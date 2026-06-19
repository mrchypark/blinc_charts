use std::collections::BTreeSet;

use blinc_charts_desktop::gallery::{
    build_desktop_ui, build_interaction_examples_ui, coverage_matrix, desktop_window_config,
    gallery_tab_labels, interaction_demo_inventory, sample_inventory, validate_sample_models,
    ChartFamily,
};

#[test]
fn sample_inventory_covers_required_chart_families() {
    let families: BTreeSet<_> = sample_inventory()
        .into_iter()
        .map(|sample| sample.family)
        .collect();

    for required in [
        ChartFamily::LinkedLine,
        ChartFamily::LinkedArea,
        ChartFamily::LinkedBar,
        ChartFamily::MultiLine,
        ChartFamily::StackedArea,
        ChartFamily::Scatter,
        ChartFamily::Heatmap,
        ChartFamily::Contour,
        ChartFamily::DensityMap,
        ChartFamily::Candlestick,
        ChartFamily::Histogram,
        ChartFamily::Statistics,
        ChartFamily::Gauge,
        ChartFamily::Funnel,
        ChartFamily::Polar,
        ChartFamily::Geo,
        ChartFamily::Hierarchy,
        ChartFamily::Network,
    ] {
        assert!(
            families.contains(&required),
            "missing required chart family: {required:?}"
        );
    }
}

#[test]
fn sample_models_construct_successfully() {
    let report = validate_sample_models().expect("sample chart constructors should accept data");

    assert_eq!(report.linked_charts, 3);
    assert!(report.total_samples >= 10);
    assert!(report.total_points >= 300);
}

#[test]
fn sample_inventory_includes_detail_pane_metadata() {
    for sample in sample_inventory() {
        assert!(
            !sample.summary.is_empty(),
            "missing summary for {:?}",
            sample.family
        );
        assert!(
            !sample.code_snippet.is_empty(),
            "missing code snippet for {:?}",
            sample.family
        );
        assert!(
            !sample.variants.is_empty(),
            "missing variants for {:?}",
            sample.family
        );
        assert!(
            !sample.interactions.is_empty(),
            "missing interaction chips for {:?}",
            sample.family
        );
        assert!(
            !sample.interaction_variants.is_empty(),
            "missing interaction variants for {:?}",
            sample.family
        );
        assert!(
            !sample.explanation.is_empty(),
            "missing explanation for {:?}",
            sample.family
        );
    }
}

#[test]
fn gallery_tabs_match_sample_sections() {
    assert_eq!(
        gallery_tab_labels(),
        vec!["Example", "Code", "Variants", "Guide"]
    );
}

#[test]
fn coverage_matrix_has_example_only_generation_tasks_for_each_family() {
    let cases = coverage_matrix();
    let covered_families: BTreeSet<_> = cases.iter().map(|case| case.family).collect();
    let sample_families: BTreeSet<_> = sample_inventory()
        .into_iter()
        .map(|sample| sample.family)
        .collect();

    assert_eq!(covered_families, sample_families);
    assert!(cases.len() >= sample_families.len());

    for case in cases {
        assert!(
            case.task
                .starts_with("Using only the provided blinc_charts examples"),
            "case task must constrain mini-model input: {case:?}"
        );
        assert!(
            !case.variant.is_empty() && !case.interaction.is_empty(),
            "case must name variant and interaction: {case:?}"
        );
        assert!(
            case.evidence.contains("Chart") || case.evidence.contains("chart"),
            "case evidence should include chart construction clues: {case:?}"
        );
    }
}

#[test]
fn coverage_matrix_families_build_runnable_interaction_ui() {
    let families: BTreeSet<_> = coverage_matrix()
        .into_iter()
        .map(|case| case.family)
        .collect();

    for family in families {
        let _ui: blinc_layout::div::Div = build_interaction_examples_ui(family)
            .unwrap_or_else(|err| panic!("coverage matrix family {family:?} should build: {err}"));
    }
}

#[test]
fn interactive_samples_document_bindings_and_gestures() {
    let combined = sample_inventory()
        .into_iter()
        .filter(|sample| {
            matches!(
                sample.family,
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
        })
        .flat_map(|sample| sample.interaction_variants.iter().copied())
        .map(|note| format!("{} {} {}", note.0, note.1, note.2))
        .collect::<Vec<_>>()
        .join("\n");

    for keyword in ["ChartInputBindings", "DragBinding", "pan", "brush"] {
        assert!(
            combined.contains(keyword),
            "interactive variants should mention {keyword}"
        );
    }
}

#[test]
fn interaction_gallery_exposes_runnable_examples() {
    let linked_line = interaction_demo_inventory(ChartFamily::LinkedLine);
    let histogram = interaction_demo_inventory(ChartFamily::Histogram);
    let contour = interaction_demo_inventory(ChartFamily::Contour);
    let heatmap = interaction_demo_inventory(ChartFamily::Heatmap);
    let polar = interaction_demo_inventory(ChartFamily::Polar);

    assert!(
        linked_line
            .iter()
            .any(|demo| demo.title == "Linked domain sync"),
        "linked line should expose linked-domain interaction"
    );
    assert!(
        !histogram
            .iter()
            .any(|demo| demo.title == "Linked domain sync"),
        "histogram should not advertise linked-domain sync"
    );
    assert!(
        contour
            .iter()
            .any(|demo| demo.title == "2D pan + wheel/pinch zoom + rectangle brush"),
        "contour should expose 2D surface navigation"
    );
    assert!(
        heatmap
            .iter()
            .any(|demo| demo.title == "Static/model-driven render"),
        "heatmap should expose model-driven rendering"
    );
    assert!(
        polar
            .iter()
            .any(|demo| demo.title == "Hover-only inspection"),
        "polar should expose hover-only inspection"
    );

    let combined = linked_line
        .iter()
        .chain(histogram.iter())
        .chain(contour.iter())
        .chain(heatmap.iter())
        .chain(polar.iter())
        .map(|demo| {
            format!(
                "{} {} {} {}",
                demo.title, demo.instruction, demo.code_change, demo.effect
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let pan_zoom = linked_line
        .iter()
        .find(|demo| demo.title == "Hover + pan + wheel/pinch zoom")
        .expect("linked line should include a pan/zoom example");
    assert!(
        pan_zoom.code_change.contains("scroll_zoom = true"),
        "pan/zoom example should enable chart-local wheel zoom"
    );

    for keyword in [
        "Shift+drag",
        "DragBinding::none",
        "linked_line_chart_with_bindings",
        "histogram_chart_with_bindings",
        "contour_chart",
        "polar_chart",
        "heatmap_chart",
    ] {
        assert!(
            combined.contains(keyword),
            "interaction examples should include {keyword}"
        );
    }

    for family in [
        ChartFamily::LinkedLine,
        ChartFamily::Histogram,
        ChartFamily::Contour,
        ChartFamily::Heatmap,
        ChartFamily::Polar,
    ] {
        let _ui: blinc_layout::div::Div = build_interaction_examples_ui(family)
            .expect("interaction examples should build live charts");
    }
}

#[test]
fn desktop_ui_builds_without_running_native_window() {
    let _ui: blinc_layout::div::Div =
        build_desktop_ui(1280.0, 860.0).expect("desktop gallery UI should build on host");
}

#[test]
fn desktop_window_config_describes_resizable_native_gallery() {
    let config = desktop_window_config();

    assert_eq!(config.title, "blinc_charts desktop gallery");
    assert_eq!(config.width, 1280);
    assert_eq!(config.height, 860);
    assert!(config.resizable);
}
