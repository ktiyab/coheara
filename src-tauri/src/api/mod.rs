//! M0-01: Mobile API Router.
//!
//! Exposes desktop business logic as HTTP endpoints for the mobile
//! companion app. Routes are nested under `/api/` and protected by
//! a middleware stack: Rate Limit → Nonce → Auth → Audit → Handler.
//!
//! The router is composable — `mobile_api_router()` returns a `Router`
//! that can be mounted on any axum server instance.

pub mod endpoints;
pub mod error;
pub mod middleware;
pub mod router;
pub mod server;
pub mod types;
pub mod websocket;

pub use router::mobile_api_router;
pub use server::{MobileApiServer, MobileApiSession, MobileApiStatus};
pub use types::ApiContext;
