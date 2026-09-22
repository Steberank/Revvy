mod api;
mod bots;
mod cache;
mod config;
mod db;
mod game;
mod maps_storage;
mod network;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    config::init_tracing();
    let config = config::ServerConfig::load()?;
    tracing::info!(
        http = %config.http_addr,
        quic = %config.quic_addr,
        "servidor Revvy"
    );

    let http = api::serve(&config.http_addr);
    let quic = network::serve(&config.quic_addr);
    tokio::select! {
        result = http => result?,
        result = quic => result?,
    }
    Ok(())
}
