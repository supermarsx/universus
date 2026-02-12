//! Core building blocks for the platform-db crate.

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-db"
}
