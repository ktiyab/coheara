//! ME-01: State module — re-exports CoreState as the managed type.
//!
//! All command handlers use `State<'_, Arc<CoreState>>`.
//! This module exists for backward compatibility with `commands::state::*` imports.

pub use crate::core_state::CoreState;
