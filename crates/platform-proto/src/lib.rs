//! Core building blocks for the platform-proto crate.

/// Returns the crate name for a basic compile-time sanity check.
pub const fn crate_name() -> &'static str {
    "platform-proto"
}
