//! Mobile API server lifecycle — starts/stops the axum HTTP server
//! that serves the mobile companion API.
//!
//! The server binds to the local network IP on an ephemeral port,
//! mounts `mobile_api_router()` (M0-01), and runs alongside Tauri.
//! Mobile devices discover the server via QR code during pairing.
//!
//! Pattern mirrors `wifi_transfer.rs` and `distribution.rs`:
//! bind → spawn background task → return handle with shutdown channel.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::api::router::mobile_api_router;
use crate::core_state::CoreState;
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
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MobileApiServer {
    /// Shut down the server gracefully.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            tracing::info!("Mobile API server shutdown signal sent");
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Server lifecycle
// ═══════════════════════════════════════════════════════════

/// Start the mobile API server on the local network.
///
/// Binds to the local IP on an ephemeral port, builds the full
/// `mobile_api_router` with middleware stack, and spawns the
/// axum server in a background tokio task.
///
/// Returns a `MobileApiServer` handle with session metadata
/// and a shutdown channel.
pub async fn start_mobile_api_server(
    core: Arc<CoreState>,
) -> Result<MobileApiServer, String> {
    // 1. Detect local IP
    let local_ip = local_ip_address::local_ip()
        .map_err(|e| format!("Cannot detect local IP: {e}"))?;

    if !is_local_network(&local_ip) {
        return Err(
            "Not on a local network. Mobile API requires a local network connection.".into(),
        );
    }

    start_mobile_api_server_on(core, local_ip).await
}

/// Start the mobile API server on a specific IP address.
///
/// Factored out from `start_mobile_api_server` to allow testing
/// with `127.0.0.1` (which isn't "local network" by the private
/// IP check, but works in tests).
pub async fn start_mobile_api_server_on(
    core: Arc<CoreState>,
    ip: IpAddr,
) -> Result<MobileApiServer, String> {
    // 1. Bind to ephemeral port
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(ip, 0))
        .await
        .map_err(|e| format!("Failed to bind mobile API server: {e}"))?;

    let addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to get server address: {e}"))?;

    tracing::info!(%addr, "Mobile API server binding");

    // 2. Build the router (M0-01: full middleware stack)
    let app = mobile_api_router(core);

    // 3. Create session metadata
    let session = MobileApiSession {
        session_id: Uuid::new_v4().to_string(),
        server_addr: addr.to_string(),
        port: addr.port(),
        started_at: chrono::Utc::now().to_rfc3339(),
    };

    // 4. Set up shutdown signal
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // 5. Spawn server in background task
    tokio::spawn(async move {
        let shutdown_signal = async move {
            let _ = shutdown_rx.await;
            tracing::info!("Mobile API server received shutdown signal");
        };

        tracing::info!(%addr, "Mobile API server started");

        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
        {
            tracing::error!("Mobile API server error: {e}");
        }

        tracing::info!("Mobile API server stopped");
    });

    Ok(MobileApiServer {
        session,
        shutdown_tx: Some(shutdown_tx),
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
        let mut server = start_mobile_api_server_on(
            core,
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        )
        .await
        .expect("server should start");

        assert!(!server.session.session_id.is_empty());
        assert!(server.session.port > 0);

        // Health check via HTTP — without auth/nonce headers, should be rejected
        let url = format!("http://127.0.0.1:{}/api/health", server.session.port);
        let resp = reqwest::get(&url).await.unwrap();
        // Middleware rejects: 400 (missing nonce) or 401 (missing auth)
        assert!(
            resp.status().is_client_error(),
            "Expected 4xx, got {}",
            resp.status()
        );

        server.shutdown();
        // Give server time to stop
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn server_session_has_valid_metadata() {
        let core = test_core();
        let mut server = start_mobile_api_server_on(
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
        let mut server = start_mobile_api_server_on(
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
        // Should reach handler (not 401) — may be 401 from pairing logic but not from auth middleware
        assert_ne!(resp.status(), reqwest::StatusCode::NOT_FOUND);

        server.shutdown();
    }

    #[tokio::test]
    async fn shutdown_is_idempotent() {
        let core = test_core();
        let mut server = start_mobile_api_server_on(
            core,
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        )
        .await
        .expect("server should start");

        server.shutdown();
        server.shutdown(); // Second call should be safe
    }
}
