//! Mobile API server lifecycle — starts/stops the axum HTTPS server
//! that serves the mobile companion API.
//!
//! SEC-HTTPS-01: The server uses TLS via a locally-generated Certificate
//! Authority (see `local_ca` module). Mobile devices trust the CA by
//! installing a `.mobileconfig` profile (iOS) or PEM (Android).
//!
//! Pattern: Home Assistant local API + Synology DSM CA trust model.
//! Bind → TLS config → spawn background task → return handle.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::api::router::mobile_api_router;
use crate::core_state::CoreState;
use crate::local_ca::ServerCertBundle;
use crate::wifi_transfer::is_local_network;

// ═══════════════════════════════════════════════════════════
// Public types
// ═══════════════════════════════════════════════════════════

/// Session metadata for a running mobile API server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileApiSession {
    pub session_id: String,
    pub server_addr: String,
    pub port: u16,
    pub started_at: String,
}

/// Status returned by `get_mobile_api_status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileApiStatus {
    pub running: bool,
    pub session: Option<MobileApiSession>,
}

/// Handle to a running mobile API server. Stored in `CoreState`.
pub struct MobileApiServer {
    pub session: MobileApiSession,
    /// SHA-256 fingerprint of the server TLS certificate (for QR cert pinning).
    pub cert_fingerprint: Option<String>,
    shutdown: ShutdownMechanism,
}

/// Shutdown mechanism — differs between HTTPS (Handle) and HTTP (oneshot).
#[allow(dead_code)] // Oneshot variant used only in tests
enum ShutdownMechanism {
    /// Plain HTTP server (tests only) — shutdown via oneshot channel.
    Oneshot(Option<oneshot::Sender<()>>),
    /// HTTPS server (production) — shutdown via axum-server Handle.
    Handle(axum_server::Handle),
}

impl MobileApiServer {
    /// Shut down the server gracefully.
    pub fn shutdown(&mut self) {
        match &mut self.shutdown {
            ShutdownMechanism::Oneshot(tx) => {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(());
                    tracing::info!("Mobile API server shutdown signal sent (HTTP)");
                }
            }
            ShutdownMechanism::Handle(handle) => {
                handle.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
                tracing::info!("Mobile API server shutdown signal sent (HTTPS)");
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Server lifecycle
// ═══════════════════════════════════════════════════════════

/// Start the mobile API server with HTTPS on the local network.
///
/// Detects the local IP, validates it's a private network, then delegates
/// to `start_https_server` with the provided TLS certificate bundle.
pub async fn start_mobile_api_server(
    core: Arc<CoreState>,
    cert: ServerCertBundle,
) -> Result<MobileApiServer, String> {
    let local_ip = local_ip_address::local_ip()
        .map_err(|e| format!("Cannot detect local IP: {e}"))?;

    if !is_local_network(&local_ip) {
        return Err(
            "Not on a local network. Mobile API requires a local network connection.".into(),
        );
    }

    start_https_server(core, local_ip, cert).await
}

/// Start the HTTPS server on a specific IP with TLS.
///
/// Uses `axum-server` with rustls for TLS termination. The cert chain
/// includes both the server certificate and the CA certificate, so
/// clients that installed the CA can validate the full chain.
///
/// Ephemeral port binding: binds to port 0, then `Handle::listening()`
/// resolves to the actual assigned port.
async fn start_https_server(
    core: Arc<CoreState>,
    ip: IpAddr,
    cert: ServerCertBundle,
) -> Result<MobileApiServer, String> {
    use axum_server::tls_rustls::RustlsConfig;

    // Rustls 0.23 requires an explicit crypto provider (ring or aws-lc-rs).
    // Install ring as process-level default; `.ok()` ignores if already set.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    // Build TLS config: cert chain (server + CA) + server private key
    let cert_chain = vec![cert.cert_der.clone(), cert.ca_cert_der.clone()];
    let rustls_config = RustlsConfig::from_der(cert_chain, cert.key_der.clone())
        .await
        .map_err(|e| format!("Failed to build TLS config: {e}"))?;

    let handle = axum_server::Handle::new();
    let addr = SocketAddr::new(ip, 0);
    let app = mobile_api_router(core);
    let fingerprint = cert.fingerprint.clone();

    let server_handle = handle.clone();
    tokio::spawn(async move {
        tracing::info!(%addr, "Mobile API HTTPS server starting");

        if let Err(e) = axum_server::bind_rustls(addr, rustls_config)
            .handle(server_handle)
            .serve(app.into_make_service())
            .await
        {
            tracing::error!("Mobile API HTTPS server error: {e}");
        }

        tracing::info!("Mobile API HTTPS server stopped");
    });

    // Wait for the server to bind and discover the actual port
    let actual_addr = handle
        .listening()
        .await
        .ok_or("HTTPS server failed to bind")?;
    tracing::info!(%actual_addr, "Mobile API HTTPS server bound");

    let session = MobileApiSession {
        session_id: Uuid::new_v4().to_string(),
        server_addr: actual_addr.to_string(),
        port: actual_addr.port(),
        started_at: chrono::Utc::now().to_rfc3339(),
    };

    Ok(MobileApiServer {
        session,
        cert_fingerprint: Some(fingerprint),
        shutdown: ShutdownMechanism::Handle(handle),
    })
}

/// Start a plain HTTP server (for tests only — no TLS).
///
/// Preserves the original `axum::serve()` pattern with oneshot shutdown.
/// Production code should always use `start_https_server()`.
#[cfg(test)]
pub async fn start_http_server(
    core: Arc<CoreState>,
    ip: IpAddr,
) -> Result<MobileApiServer, String> {
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(ip, 0))
        .await
        .map_err(|e| format!("Failed to bind mobile API server: {e}"))?;

    let addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to get server address: {e}"))?;

    let app = mobile_api_router(core);

    let session = MobileApiSession {
        session_id: Uuid::new_v4().to_string(),
        server_addr: addr.to_string(),
        port: addr.port(),
        started_at: chrono::Utc::now().to_rfc3339(),
    };

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let shutdown_signal = async move {
            let _ = shutdown_rx.await;
        };

        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
        {
            tracing::error!("Mobile API server error: {e}");
        }
    });

    Ok(MobileApiServer {
        session,
        cert_fingerprint: None,
        shutdown: ShutdownMechanism::Oneshot(Some(shutdown_tx)),
    })
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_core() -> Arc<CoreState> {
        Arc::new(CoreState::new())
    }

    #[tokio::test]
    async fn start_and_stop_server() {
        let core = test_core();
        let mut server = start_http_server(
            core,
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        )
        .await
        .expect("server should start");

        assert!(!server.session.session_id.is_empty());
        assert!(server.session.port > 0);
        assert!(server.cert_fingerprint.is_none()); // HTTP mode — no cert

        // Health check via HTTP — without auth/nonce headers, should be rejected
        let url = format!("http://127.0.0.1:{}/api/health", server.session.port);
        let resp = reqwest::get(&url).await.unwrap();
        assert!(
            resp.status().is_client_error(),
            "Expected 4xx, got {}",
            resp.status()
        );

        server.shutdown();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn server_session_has_valid_metadata() {
        let core = test_core();
        let mut server = start_http_server(
            core,
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        )
        .await
        .expect("server should start");

        assert!(!server.session.started_at.is_empty());
        assert!(server.session.server_addr.contains(':'));

        server.shutdown();
    }

    #[tokio::test]
    async fn server_serves_api_routes() {
        let core = test_core();
        let mut server = start_http_server(
            core,
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        )
        .await
        .expect("server should start");

        let port = server.session.port;

        // Unknown route returns 404
        let url = format!("http://127.0.0.1:{port}/nonexistent");
        let resp = reqwest::get(&url).await.unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);

        // Auth pair endpoint (unprotected, rate-limited only) — should not 401
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/api/auth/pair"))
            .header("Content-Type", "application/json")
            .body(r#"{"token":"invalid","phone_pubkey":"dGVzdA==","device_name":"Test","device_model":"Test"}"#)
            .send()
            .await
            .unwrap();
        assert_ne!(resp.status(), reqwest::StatusCode::NOT_FOUND);

        server.shutdown();
    }

    #[tokio::test]
    async fn shutdown_is_idempotent() {
        let core = test_core();
        let mut server = start_http_server(
            core,
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        )
        .await
        .expect("server should start");

        server.shutdown();
        server.shutdown(); // Second call should be safe
    }

    #[tokio::test]
    async fn https_server_starts_with_cert() {
        use crate::local_ca;

        let core = test_core();
        let ca = local_ca::generate_ca().expect("CA generation should succeed");
        let ip = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
        let cert = local_ca::issue_server_cert(&ca, ip)
            .expect("Server cert should be issued");

        let mut server = start_https_server(core, ip, cert)
            .await
            .expect("HTTPS server should start");

        assert!(server.cert_fingerprint.is_some());
        assert!(server.session.port > 0);

        // Verify the server is listening on HTTPS (plain HTTP request should fail)
        let url = format!("http://127.0.0.1:{}/api/health", server.session.port);
        let result = reqwest::get(&url).await;
        // Plain HTTP to an HTTPS port should fail (connection error or protocol mismatch)
        assert!(result.is_err() || result.unwrap().status().is_server_error());

        server.shutdown();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
