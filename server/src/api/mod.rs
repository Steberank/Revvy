//! HTTP (Axum). En la fase 0 solo responde el health check.

mod auth;
mod maps;
mod stats;

use tokio::net::TcpListener;

pub fn router() -> axum::Router {
    axum::Router::new().route("/health", axum::routing::get(health))
}

async fn health() -> &'static str {
    "ok"
}

pub async fn serve(addr: &str) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    tracing::info!(%local, "HTTP escuchando");
    axum::serve(listener, router()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn health_responds_ok() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router()).await.unwrap();
        });

        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut buf = vec![0_u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("200"), "{response}");
        assert!(response.contains("ok"), "{response}");
    }
}
