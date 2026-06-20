fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    blinc_app::windowed::WindowedApp::run(
        blinc_charts_desktop::gallery::desktop_window_config(),
        blinc_charts_desktop::gallery::build_native_ui,
    )
    .map_err(Into::into)
}
