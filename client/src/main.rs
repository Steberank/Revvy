mod app;
mod assets_pipeline;
mod audio;
mod config;
mod ecs;
mod input;
mod network;
mod platform;
mod render;
mod ui;

#[cfg(all(
    feature = "track-editor",
    not(any(target_os = "android", target_os = "ios"))
))]
mod editor;

fn main() -> anyhow::Result<()> {
    config::init_tracing();
    let config = config::ClientConfig::load()?;
    tracing::info!(
        title = %config.window_title,
        width = config.window_width,
        height = config.window_height,
        "cliente Revvy"
    );
    app::run(config)
}
