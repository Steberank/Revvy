//! Quinn: el canal de juego. En la fase 0 acepta una conexión y la loguea.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

pub const ALPN: &[u8] = b"revvy";

pub struct QuicEndpoint {
    pub endpoint: quinn::Endpoint,
    /// Certificado efímero. Lo lee el test de handshake para armar el trust store.
    #[cfg_attr(not(test), allow(dead_code))]
    pub cert_der: CertificateDer<'static>,
}

pub fn bind(addr: SocketAddr) -> anyhow::Result<QuicEndpoint> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
            .context("no se pudo generar el certificado efímero")?;
    let cert_der = CertificateDer::from(cert);
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));

    let mut rustls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key)
        .context("certificado QUIC inválido")?;
    rustls_config.alpn_protocols = vec![ALPN.to_vec()];

    let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(rustls_config)
        .context("no se pudo armar la config QUIC")?;
    let server_config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    let endpoint = quinn::Endpoint::server(server_config, addr)?;

    Ok(QuicEndpoint { endpoint, cert_der })
}

pub async fn serve(addr: &str) -> anyhow::Result<()> {
    let addr: SocketAddr = addr.parse().context("quic_addr inválida")?;
    let quic = bind(addr)?;
    let local = quic.endpoint.local_addr()?;
    tracing::info!(%local, "QUIC escuchando");
    accept_loop(quic.endpoint).await
}

async fn accept_loop(endpoint: quinn::Endpoint) -> anyhow::Result<()> {
    while let Some(incoming) = endpoint.accept().await {
        match incoming.await {
            Ok(conn) => {
                tracing::info!(remote = %conn.remote_address(), "conexión QUIC aceptada");
            }
            Err(err) => {
                tracing::warn!(%err, "handshake QUIC falló");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::RootCertStore;

    #[tokio::test]
    async fn accepts_one_connection() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let quic = bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let server_addr = quic.endpoint.local_addr().unwrap();
        let cert = quic.cert_der.clone();

        let accepted = tokio::spawn(async move {
            let incoming = quic.endpoint.accept().await.expect("incoming");
            let conn = incoming.await.expect("handshake");
            tracing::info!(remote = %conn.remote_address(), "conexión QUIC aceptada");
            conn.remote_address()
        });

        let mut roots = RootCertStore::empty();
        roots.add(cert).unwrap();
        let mut client_crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        client_crypto.alpn_protocols = vec![ALPN.to_vec()];
        let quic_client = quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
            .expect("client crypto");
        let mut client = quinn::Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
        client.set_default_client_config(quinn::ClientConfig::new(Arc::new(quic_client)));

        client
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .expect("connect");

        let remote = tokio::time::timeout(std::time::Duration::from_secs(5), accepted)
            .await
            .expect("timeout")
            .expect("accept task");
        assert_eq!(remote.ip().to_string(), "127.0.0.1");
    }
}
