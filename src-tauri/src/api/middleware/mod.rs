//! M0-01: API middleware stack.
//!
//! Execution order (outermost → innermost):
//! 1. Rate limiter — reject early, save resources
//! 2. Nonce verifier — anti-replay
//! 3. Auth validator — token validation + rotation
//! 4. Audit logger — logs after auth, has device_id

pub mod audit;
pub mod auth;
pub mod nonce;
pub mod rate;
