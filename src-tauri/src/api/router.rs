//! M0-01: Mobile API router.
//!
//! Returns a composable `Router` that can be mounted on any axum server.
//! Routes are nested under `/api/`.
//!
//! Middleware stack (outermost → innermost):
//! 1. Rate limiter → 2. Nonce verifier → 3. Auth validator → 4. Audit logger

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::api::websocket;

use crate::api::endpoints;
use crate::api::middleware;
use crate::api::types::ApiContext;
use crate::core_state::CoreState;

/// Build the mobile API router.
///
/// Returns a `Router` with all mobile endpoints under `/api/`.
/// All endpoints require bearer token authentication.
///
/// Middleware uses `Extension<ApiContext>` (injected as the outermost layer).
/// Endpoint handlers use `State<ApiContext>` (provided via `with_state`).
pub fn mobile_api_router(core: Arc<CoreState>) -> Router {
    let ctx = ApiContext::new(core);
    build_router(ctx)
}

/// Build router from pre-constructed `ApiContext`.
///
/// Used by integration tests that need access to the shared `ApiContext`
/// (e.g. to issue WS tickets directly).
#[cfg(test)]
pub(crate) fn mobile_api_router_with_ctx(ctx: ApiContext) -> Router {
    build_router(ctx)
}

fn build_router(ctx: ApiContext) -> Router {
    // Protected routes — require auth + full middleware stack
    //
    // Layers are applied from bottom (innermost) to top (outermost):
    //   Extension (outermost) → Rate limit → Nonce → Auth → Audit (innermost) → Handler
    //
    // Extension must be outermost so all middleware can access ApiContext.
    // Routes with state — .with_state() converts Router<ApiContext> → Router<()>
    // so middleware layers (which use from_fn with state=()) are compatible.
    //
    // NOTE: Path params use `:param` syntax (matchit 0.7 / axum 0.7).
    let protected = Router::new()
        .route("/health", get(endpoints::health::check))
        .route("/home", get(endpoints::home::dashboard))
        .route("/medications", get(endpoints::medications::list))
        .route("/medications/:id", get(endpoints::medications::detail))
        .route("/labs", get(endpoints::labs::list))
        .route("/labs/recent", get(endpoints::labs::recent))
        .route(
            "/labs/history/:test_name",
            get(endpoints::labs::history),
        )
        .route("/alerts/critical", get(endpoints::alerts::critical))
        .route("/chat/send", post(endpoints::chat::send))
        .route(
            "/chat/suggestions",
            get(endpoints::chat::suggestions),
        )
        .route("/chat/conversations", get(endpoints::chat::conversations))
        .route(
            "/chat/conversations/:id",
            get(endpoints::chat::conversation),
        )
        .route("/journal/record", post(endpoints::journal::record))
        .route("/journal/history", get(endpoints::journal::history))
        .route("/timeline/recent", get(endpoints::timeline::recent))
        .route("/appointments", get(endpoints::appointments::list))
        .route(
            "/appointments/:id/prep",
            get(endpoints::appointments::prep),
        )
        .route("/documents/upload", post(endpoints::documents::upload))
        .route("/sync", post(endpoints::sync::delta))
        .route("/auth/ws-ticket", post(endpoints::auth::ws_ticket))
        .with_state(ctx.clone())
        // Middleware stack (innermost first, outermost last):
        .layer(axum::middleware::from_fn(middleware::audit::log_access))
        .layer(axum::middleware::from_fn(middleware::auth::require_auth))
        .layer(axum::middleware::from_fn(middleware::nonce::verify_nonce))
        .layer(axum::middleware::from_fn(middleware::rate::limit))
        // Extension must be outermost so middleware can extract ApiContext
        .layer(axum::Extension(ctx.clone()));

    // Unprotected routes (rate-limited only, no auth required)
    let unprotected = Router::new()
        .route("/auth/pair", post(endpoints::auth::pair))
        .with_state(ctx.clone())
        .layer(axum::middleware::from_fn(middleware::rate::limit))
        .layer(axum::Extension(ctx.clone()));

    // WebSocket upgrade route (ticket-based auth, rate-limited)
    let ws_routes = Router::new()
        .route("/ws/connect", get(websocket::ws_upgrade))
        .with_state(ctx.clone())
        .layer(axum::middleware::from_fn(middleware::rate::limit))
        .layer(axum::Extension(ctx));

    // Mount all routes
    Router::new()
        .nest("/api", protected)
        .nest("/api", unprotected)
        .merge(ws_routes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use base64::Engine;
    use tower::ServiceExt;

    use crate::api::types::{generate_token, hash_token};

    fn test_core_state() -> Arc<CoreState> {
        Arc::new(CoreState::new())
    }

    fn test_core_state_with_device() -> (Arc<CoreState>, String) {
        let state = Arc::new(CoreState::new());
        let token = generate_token();
        let hash = hash_token(&token);

        {
            let mut devices = state.write_devices().unwrap();
            devices
                .register_device(
                    "test-device".to_string(),
                    "Test Phone".to_string(),
                    "TestModel".to_string(),
                    hash,
                )
                .unwrap();
        }

        (state, token)
    }

    /// Create a CoreState backed by a temp profile directory with an active session + DB.
    /// Returns (Arc<CoreState>, token, _tempdir_guard).
    /// The tempdir guard must be kept alive for the duration of the test.
    fn test_core_state_with_profile() -> (Arc<CoreState>, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let mut core = CoreState::new();
        core.profiles_dir = tmp.path().to_path_buf();
        let core = Arc::new(core);

        // Create a real profile with password
        let (info, _phrase) = crate::crypto::profile::create_profile(
            &core.profiles_dir,
            "TestPatient",
            "test-password-123",
            None,
            None,
            None,
            None,
        )
        .unwrap();

        // Open the profile to get a session
        let session = crate::crypto::profile::open_profile(
            &core.profiles_dir,
            &info.id,
            "test-password-123",
        )
        .unwrap();
        core.set_session(session).unwrap();

        // Register a paired device
        let token = generate_token();
        let hash = hash_token(&token);
        {
            let mut devices = core.write_devices().unwrap();
            devices
                .register_device(
                    "test-device".to_string(),
                    "Test Phone".to_string(),
                    "TestModel".to_string(),
                    hash,
                )
                .unwrap();
        }

        (core, token, tmp)
    }

    fn make_request(
        method: &str,
        uri: &str,
        token: Option<&str>,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header(
                "X-Request-Timestamp",
                chrono::Utc::now().timestamp().to_string(),
            );

        if let Some(t) = token {
            builder = builder.header("Authorization", format!("Bearer {t}"));
        }

        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn health_requires_auth() {
        let core = test_core_state();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/health", None);
        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_succeeds_with_valid_token() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/health", Some(&token));
        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Should have X-New-Token header (rotated token)
        assert!(response.headers().contains_key("X-New-Token"));
        // Should have Cache-Control: no-store
        assert_eq!(
            response.headers().get("Cache-Control").unwrap(),
            "no-store"
        );
    }

    #[tokio::test]
    async fn home_returns_503_when_no_profile() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/home", Some(&token));
        let response = app.oneshot(req).await.unwrap();

        // No active profile session → 503 PROFILE_LOCKED
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn invalid_token_returns_401() {
        let core = test_core_state();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/health", Some("invalid-token"));
        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rotated_token_works_for_next_request() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core.clone());

        // First request with original token
        let req = make_request("GET", "/api/health", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Extract rotated token
        let new_token = response
            .headers()
            .get("X-New-Token")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        // Second request with rotated token
        let app2 = mobile_api_router(core);
        let req2 = make_request("GET", "/api/health", Some(&new_token));
        let response2 = app2.oneshot(req2).await.unwrap();
        assert_eq!(response2.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn old_token_works_during_grace_period() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core.clone());

        // First request rotates the token
        let req = make_request("GET", "/api/health", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Old token should still work during grace period
        let app2 = mobile_api_router(core);
        let req2 = make_request("GET", "/api/health", Some(&token));
        let response2 = app2.oneshot(req2).await.unwrap();
        assert_eq!(response2.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_nonce_returns_400() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core);

        // Request without nonce headers
        let req = Request::builder()
            .method("GET")
            .uri("/api/health")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn not_found_for_unknown_route() {
        let core = test_core_state();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/nonexistent", Some("token"));
        let response = app.oneshot(req).await.unwrap();

        // axum returns 404 for unknown routes before any middleware runs
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn medications_returns_503_when_no_profile() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/medications", Some(&token));
        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn ws_ticket_requires_auth() {
        let core = test_core_state();
        let app = mobile_api_router(core);

        // Includes nonce headers but no auth token → passes nonce, fails auth
        let req = make_request("POST", "/api/auth/ws-ticket", None);
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn ws_ticket_returns_ticket_with_auth() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core);

        let req = make_request("POST", "/api/auth/ws-ticket", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(!json["ticket"].as_str().unwrap().is_empty());
        assert_eq!(json["expires_in"], 30);
    }

    #[tokio::test]
    async fn chat_send_validates_empty_message() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core);

        let req = Request::builder()
            .method("POST")
            .uri("/api/chat/send")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header(
                "X-Request-Timestamp",
                chrono::Utc::now().timestamp().to_string(),
            )
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"message":""}"#))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        // Should reach the handler and return 400 for empty message
        // (or 503 if no profile — depends on middleware order)
        let status = response.status();
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::SERVICE_UNAVAILABLE,
            "Expected 400 or 503, got {status}"
        );
    }

    // ── RS-M0-01-005: Endpoint handler response shape tests ──────

    async fn response_json(response: axum::http::Response<axum::body::Body>) -> serde_json::Value {
        let body = axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn health_response_shape() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/health", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["status"], "ok");
        assert!(json["profile_active"].is_boolean());
        assert!(json["version"].is_string());
        assert!(!json["version"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn health_response_shape_with_active_profile() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/health", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["profile_active"], true);
    }

    #[tokio::test]
    async fn home_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/home", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["profile_name"], "TestPatient");
        assert!(json["stats"].is_object(), "stats should be an object");
        assert!(json["recent_documents"].is_array(), "recent_documents should be array");
        assert!(json["onboarding"].is_object(), "onboarding should be object");
        assert!(json["critical_alerts"].is_array(), "critical_alerts should be array");
        assert!(json["last_sync"].is_string(), "last_sync should be string");
    }

    #[tokio::test]
    async fn medications_list_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/medications", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["profile_name"], "TestPatient");
        assert!(json["medications"].is_array());
        assert!(json["total_active"].is_number());
        assert!(json["total_paused"].is_number());
        assert!(json["total_stopped"].is_number());
        assert!(json["prescribers"].is_array());
        assert!(json["last_updated"].is_string());
    }

    #[tokio::test]
    async fn labs_recent_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/labs/recent", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["profile_name"], "TestPatient");
        assert!(json["results"].is_array());
        assert!(json["last_updated"].is_string());
    }

    #[tokio::test]
    async fn alerts_critical_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/alerts/critical", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert!(json["alerts"].is_array());
    }

    #[tokio::test]
    async fn chat_send_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = Request::builder()
            .method("POST")
            .uri("/api/chat/send")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header("X-Request-Timestamp", chrono::Utc::now().timestamp().to_string())
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"message":"What medications am I taking?"}"#))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert!(json["conversation_id"].is_string());
        assert!(!json["conversation_id"].as_str().unwrap().is_empty());
        assert!(json["message_id"].is_string());
        assert!(json["disclaimer"].is_string());
    }

    #[tokio::test]
    async fn journal_record_validates_severity() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = Request::builder()
            .method("POST")
            .uri("/api/journal/record")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header("X-Request-Timestamp", chrono::Utc::now().timestamp().to_string())
            .header("Content-Type", "application/json")
            .body(Body::from(
                r#"{"severity":10,"category":"pain","specific":"headache","onset_date":"2025-01-01","onset_time":null,"body_region":null,"duration":null,"character":null,"aggravating":[],"relieving":[],"timing_pattern":null,"notes":null}"#,
            ))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let json = response_json(response).await;
        assert_eq!(json["error"]["code"], "BAD_REQUEST");
        assert!(json["error"]["message"].as_str().unwrap().contains("Severity"));
    }

    #[tokio::test]
    async fn error_503_response_shape() {
        let (core, token) = test_core_state_with_device();
        let app = mobile_api_router(core);

        // No active profile → 503
        let req = make_request("GET", "/api/home", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let json = response_json(response).await;
        assert_eq!(json["error"]["code"], "PROFILE_LOCKED");
        assert!(json["error"]["message"].is_string());
    }

    // ═════════════════════════════════════════════════════════
    // IMP-018: End-to-end pairing integration tests
    // ═════════════════════════════════════════════════════════

    #[tokio::test]
    async fn e2e_pairing_full_flow() {
        // 1. Create CoreState with active profile
        let (core, _existing_token, _tmp) = test_core_state_with_profile();

        // 2. Start pairing on the desktop
        let qr_data = {
            let mut pairing = core.lock_pairing().unwrap();
            let response = pairing
                .start("https://192.168.1.42:8443".to_string(), "SHA256:AB:CD".to_string())
                .unwrap();
            response.qr_data
        };

        // 3. Build the API router
        let ctx = crate::api::types::ApiContext::new(core.clone());
        let app = mobile_api_router_with_ctx(ctx);

        // 4. Phone builds pair request (simulates QR scan + key generation)
        let phone_secret = x25519_dalek::EphemeralSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);
        let phone_pubkey_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, phone_public.as_bytes());

        let pair_body = serde_json::json!({
            "token": qr_data.token,
            "phone_pubkey": phone_pubkey_b64,
            "device_name": "Test iPhone",
            "device_model": "iPhone 15 Pro"
        });

        // 5. Spawn the phone's pair request (long-polls waiting for approval)
        let core_for_approval = core.clone();
        let phone_handle = tokio::spawn(async move {
            let req = Request::builder()
                .method("POST")
                .uri("/api/auth/pair")
                .header("Content-Type", "application/json")
                .body(Body::from(pair_body.to_string()))
                .unwrap();

            app.oneshot(req).await.unwrap()
        });

        // 6. Desktop approves after a short delay (simulates user clicking approve)
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        {
            let mut pairing = core_for_approval.lock_pairing().unwrap();
            let pending = pairing.pending_approval();
            assert!(pending.is_some(), "Should have a pending approval");
            assert_eq!(pending.unwrap().device_name, "Test iPhone");
            pairing.signal_approval().unwrap();
        }

        // 7. Phone's request completes with pairing response
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            phone_handle,
        )
        .await
        .expect("phone request timed out")
        .expect("phone task panicked");

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert!(
            json["session_token"].is_string() && !json["session_token"].as_str().unwrap().is_empty(),
            "Should receive a session_token"
        );
        assert!(
            json["cache_key_encrypted"].is_string(),
            "Should receive cache_key_encrypted"
        );
        assert_eq!(json["profile_name"], "TestPatient");

        // 8. Verify device is registered in DeviceManager
        let devices = core.read_devices().unwrap();
        assert!(
            devices.device_count() >= 2,
            "Should have at least 2 paired devices (existing + new)"
        );
    }

    #[tokio::test]
    async fn e2e_pairing_denial_returns_403() {
        let (core, _existing_token, _tmp) = test_core_state_with_profile();

        // Start pairing
        let qr_data = {
            let mut pairing = core.lock_pairing().unwrap();
            let response = pairing
                .start("https://192.168.1.42:8443".to_string(), "SHA256:AB:CD".to_string())
                .unwrap();
            response.qr_data
        };

        let ctx = crate::api::types::ApiContext::new(core.clone());
        let app = mobile_api_router_with_ctx(ctx);

        // Phone submits pair request
        let phone_secret = x25519_dalek::EphemeralSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);
        let phone_pubkey_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, phone_public.as_bytes());

        let pair_body = serde_json::json!({
            "token": qr_data.token,
            "phone_pubkey": phone_pubkey_b64,
            "device_name": "Suspicious Device",
            "device_model": "Unknown"
        });

        let core_for_denial = core.clone();
        let phone_handle = tokio::spawn(async move {
            let req = Request::builder()
                .method("POST")
                .uri("/api/auth/pair")
                .header("Content-Type", "application/json")
                .body(Body::from(pair_body.to_string()))
                .unwrap();

            app.oneshot(req).await.unwrap()
        });

        // Desktop denies after short delay
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        {
            let mut pairing = core_for_denial.lock_pairing().unwrap();
            pairing.deny();
        }

        let response = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            phone_handle,
        )
        .await
        .expect("phone request timed out")
        .expect("phone task panicked");

        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "Denied pairing should return 403"
        );
    }

    #[tokio::test]
    async fn e2e_pairing_wrong_token_returns_401() {
        let (core, _existing_token, _tmp) = test_core_state_with_profile();

        // Start pairing
        {
            let mut pairing = core.lock_pairing().unwrap();
            pairing
                .start("https://192.168.1.42:8443".to_string(), "SHA256:AB:CD".to_string())
                .unwrap();
        }

        let ctx = crate::api::types::ApiContext::new(core.clone());
        let app = mobile_api_router_with_ctx(ctx);

        // Phone submits with WRONG token
        let phone_secret = x25519_dalek::EphemeralSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);
        let phone_pubkey_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, phone_public.as_bytes());

        let pair_body = serde_json::json!({
            "token": "wrong-token-value",
            "phone_pubkey": phone_pubkey_b64,
            "device_name": "Attacker Phone",
            "device_model": "Unknown"
        });

        let req = Request::builder()
            .method("POST")
            .uri("/api/auth/pair")
            .header("Content-Type", "application/json")
            .body(Body::from(pair_body.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "Wrong pairing token should return 401"
        );
    }

    #[tokio::test]
    async fn e2e_pairing_then_ws_connect() {
        // Full flow: pair → get WS ticket → connect WebSocket

        let (core, _existing_token, _tmp) = test_core_state_with_profile();

        // 1. Start pairing
        let qr_data = {
            let mut pairing = core.lock_pairing().unwrap();
            pairing
                .start("https://192.168.1.42:8443".to_string(), "SHA256:AB:CD".to_string())
                .unwrap()
                .qr_data
        };

        let ctx = crate::api::types::ApiContext::new(core.clone());

        // 2. Phone pairs (approval happens concurrently)
        let phone_secret = x25519_dalek::EphemeralSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);
        let phone_pubkey_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, phone_public.as_bytes());

        let pair_body = serde_json::json!({
            "token": qr_data.token,
            "phone_pubkey": phone_pubkey_b64,
            "device_name": "Full Flow Phone",
            "device_model": "iPhone 16"
        });

        let app = mobile_api_router_with_ctx(ctx.clone());
        let core_for_approval = core.clone();
        let phone_handle = tokio::spawn(async move {
            let req = Request::builder()
                .method("POST")
                .uri("/api/auth/pair")
                .header("Content-Type", "application/json")
                .body(Body::from(pair_body.to_string()))
                .unwrap();
            app.oneshot(req).await.unwrap()
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        {
            let mut pairing = core_for_approval.lock_pairing().unwrap();
            pairing.signal_approval().unwrap();
        }

        let pair_response = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            phone_handle,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(pair_response.status(), StatusCode::OK);

        let pair_json = response_json(pair_response).await;
        let session_token = pair_json["session_token"].as_str().unwrap();

        // 3. Phone requests WS ticket using session token
        let app2 = mobile_api_router_with_ctx(ctx.clone());
        let ws_req = Request::builder()
            .method("POST")
            .uri("/api/auth/ws-ticket")
            .header("Authorization", format!("Bearer {session_token}"))
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header("X-Request-Timestamp", chrono::Utc::now().timestamp().to_string())
            .body(Body::empty())
            .unwrap();

        let ws_response = app2.oneshot(ws_req).await.unwrap();
        assert_eq!(ws_response.status(), StatusCode::OK);

        let ws_json = response_json(ws_response).await;
        let ticket = ws_json["ticket"].as_str().unwrap();
        assert!(!ticket.is_empty(), "WS ticket should not be empty");
        assert_eq!(ws_json["expires_in"], 30);

        // 4. Phone connects WebSocket with ticket
        let app3 = mobile_api_router_with_ctx(ctx);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            axum::serve(listener, app3).await.unwrap();
        });

        let ws_url = format!("ws://127.0.0.1:{}/ws/connect?ticket={ticket}", addr.port());
        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .expect("WS connect should succeed after pairing");

        // 5. Should receive Welcome
        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            futures_util::StreamExt::next(&mut ws),
        )
        .await
        .expect("timeout waiting for Welcome")
        .expect("stream ended")
        .expect("WS error");

        let text = msg.into_text().expect("not text");
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "Welcome");
        assert_eq!(parsed["profile_name"], "TestPatient");

        let _ = futures_util::SinkExt::close(&mut ws).await;
        server.abort();
    }

    // ═════════════════════════════════════════════════════════
    // E2E-M05: Additional endpoint integration tests
    // ═════════════════════════════════════════════════════════

    #[tokio::test]
    async fn timeline_recent_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/timeline/recent", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["profile_name"], "TestPatient");
        assert!(json["events"].is_array(), "events should be array");
        assert!(json["correlations"].is_array(), "correlations should be array");
        assert!(json["date_range"].is_object(), "date_range should be object");
    }

    #[tokio::test]
    async fn appointments_list_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/appointments", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["profile_name"], "TestPatient");
        assert!(json["appointments"].is_array(), "appointments should be array");
    }

    #[tokio::test]
    async fn sync_returns_204_when_nothing_changed() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        // Send version 0 for all — but empty DB means nothing to return
        let body = r#"{"versions":{"medications":0,"labs":0,"timeline":0,"alerts":0,"appointments":0,"profile":0},"journalEntries":[]}"#;
        let req = Request::builder()
            .method("POST")
            .uri("/api/sync")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header("X-Request-Timestamp", chrono::Utc::now().timestamp().to_string())
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        // Empty DB → 204 No Content (nothing changed)
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn document_upload_requires_pages() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let body = r#"{"metadata":{"page_count":0,"device_name":"Test Phone","captured_at":"2026-01-01T00:00:00Z"},"pages":[]}"#;
        let req = Request::builder()
            .method("POST")
            .uri("/api/documents/upload")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header("X-Request-Timestamp", chrono::Utc::now().timestamp().to_string())
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let json = response_json(response).await;
        assert!(json["error"]["message"].as_str().unwrap().contains("No pages"));
    }

    #[tokio::test]
    async fn document_upload_validates_page_limit() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        // Build 11 pages (exceeds MAX_PAGES=10)
        let pages: Vec<serde_json::Value> = (0..11)
            .map(|i| serde_json::json!({
                "name": format!("page{i}"),
                "data": "data:image/jpeg;base64,/9j/4AAQ",
                "width": 100,
                "height": 100,
                "size_bytes": 100
            }))
            .collect();

        let body = serde_json::json!({
            "metadata": {
                "page_count": 11,
                "device_name": "Test Phone",
                "captured_at": "2026-01-01T00:00:00Z"
            },
            "pages": pages
        });

        let req = Request::builder()
            .method("POST")
            .uri("/api/documents/upload")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header("X-Request-Timestamp", chrono::Utc::now().timestamp().to_string())
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let json = response_json(response).await;
        assert!(json["error"]["message"].as_str().unwrap().contains("Maximum"));
    }

    #[tokio::test]
    async fn document_upload_processes_valid_jpeg() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        // Create a minimal valid JPEG as base64
        let jpeg_bytes: Vec<u8> = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
            0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
        ];
        let encoded = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
        let data_url = format!("data:image/jpeg;base64,{encoded}");

        let body = serde_json::json!({
            "metadata": {
                "page_count": 1,
                "device_name": "Test Phone",
                "captured_at": "2026-01-01T00:00:00Z"
            },
            "pages": [{
                "name": "test_page",
                "data": data_url,
                "width": 100,
                "height": 100,
                "size_bytes": jpeg_bytes.len()
            }]
        });

        let req = Request::builder()
            .method("POST")
            .uri("/api/documents/upload")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Request-Nonce", uuid::Uuid::new_v4().to_string())
            .header("X-Request-Timestamp", chrono::Utc::now().timestamp().to_string())
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["status"], "processing");
        assert!(json["message"].as_str().unwrap().contains("1 page(s)"));
        assert!(json["document_id"].is_string());
    }

    #[tokio::test]
    async fn chat_conversations_list_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/chat/conversations", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert!(json["conversations"].is_array(), "conversations should be array");
    }

    #[tokio::test]
    async fn journal_history_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/journal/history", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["profile_name"], "TestPatient");
        assert!(json["symptoms"].is_array(), "symptoms should be array");
    }

    // ═════════════════════════════════════════════════════════
    // CA-06: New endpoint integration tests
    // ═════════════════════════════════════════════════════════

    #[tokio::test]
    async fn chat_suggestions_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/chat/suggestions", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert!(json["suggestions"].is_array(), "suggestions should be array");
        let suggestions = json["suggestions"].as_array().unwrap();
        assert!(!suggestions.is_empty(), "should return at least one suggestion");
        // Each suggestion has text, category, intent
        let first = &suggestions[0];
        assert!(first["text"].is_string(), "suggestion.text should be string");
        assert!(first["category"].is_string(), "suggestion.category should be string");
        assert!(first["intent"].is_string(), "suggestion.intent should be string");
    }

    #[tokio::test]
    async fn chat_suggestions_requires_auth() {
        let core = test_core_state();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/chat/suggestions", None);
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn labs_list_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/labs", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["profile_name"], "TestPatient");
        assert!(json["results"].is_array(), "results should be array");
        assert!(json["last_updated"].is_string(), "last_updated should be string");
    }

    #[tokio::test]
    async fn path_params_require_auth() {
        // Verify parameterized routes are properly registered (now using :param syntax)
        let core = test_core_state();

        // /api/medications/:id — should require auth
        let app1 = mobile_api_router(core.clone());
        let req1 = make_request("GET", "/api/medications/test-id", None);
        let resp1 = app1.oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::UNAUTHORIZED, "medications/:id should require auth");

        // /api/labs/history/:test_name — should require auth
        let app2 = mobile_api_router(core.clone());
        let req2 = make_request("GET", "/api/labs/history/HbA1c", None);
        let resp2 = app2.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED, "labs/history/:test_name should require auth");

        // /api/chat/conversations/:id — should require auth
        let app3 = mobile_api_router(core);
        let req3 = make_request("GET", "/api/chat/conversations/test-id", None);
        let resp3 = app3.oneshot(req3).await.unwrap();
        assert_eq!(resp3.status(), StatusCode::UNAUTHORIZED, "chat/conversations/:id should require auth");
    }

    #[tokio::test]
    async fn labs_history_response_shape() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/labs/history/HbA1c", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["test_name"], "HbA1c");
        assert!(json["entries"].is_array(), "entries should be array");
    }

    #[tokio::test]
    async fn labs_history_returns_empty_for_unknown_test() {
        let (core, token, _tmp) = test_core_state_with_profile();
        let app = mobile_api_router(core);

        let req = make_request("GET", "/api/labs/history/NonExistentTest", Some(&token));
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["entries"].as_array().unwrap().len(), 0);
    }
}
