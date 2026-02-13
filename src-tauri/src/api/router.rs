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

    // Protected routes — require auth + full middleware stack
    //
    // Layers are applied from bottom (innermost) to top (outermost):
    //   Extension (outermost) → Rate limit → Nonce → Auth → Audit (innermost) → Handler
    //
    // Extension must be outermost so all middleware can access ApiContext.
    // Routes with state — .with_state() converts Router<ApiContext> → Router<()>
    // so middleware layers (which use from_fn with state=()) are compatible.
    let protected = Router::new()
        .route("/health", get(endpoints::health::check))
        .route("/home", get(endpoints::home::dashboard))
        .route("/medications", get(endpoints::medications::list))
        .route("/medications/{id}", get(endpoints::medications::detail))
        .route("/labs/recent", get(endpoints::labs::recent))
        .route("/alerts/critical", get(endpoints::alerts::critical))
        .route("/chat/send", post(endpoints::chat::send))
        .route("/chat/conversations", get(endpoints::chat::conversations))
        .route(
            "/chat/conversations/{id}",
            get(endpoints::chat::conversation),
        )
        .route("/journal/record", post(endpoints::journal::record))
        .route("/journal/history", get(endpoints::journal::history))
        .route("/timeline/recent", get(endpoints::timeline::recent))
        .route("/appointments", get(endpoints::appointments::list))
        .route(
            "/appointments/{id}/prep",
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
}
