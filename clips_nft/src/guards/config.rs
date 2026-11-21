//! Configuration update guard — restrict configuration updates to authorized
//! owners and administrators (issue #1146).
//!
//! This guard module re-exports [`crate::config_guard`] as part of the
//! centralized [`crate::guards`] framework.

pub use crate::config_guard::{
    guard_config_update, is_config_authorized, require_config_admin, require_config_owner,
    validate_config,
};
