//! Layer names + display-off layer for OLED renderers.
//!
//! MUST match [[layer]] order in config/keyboard.toml (0-indexed).
//! If that order changes, update here in lockstep — `src/status.rs` (left half)
//! reads `LAYER_NAMES`; `src/trishul.rs` (right half) reads only `DISPLAY_OFF_LAYER`.

/// Selecting this layer blanks the OLED (MEDIA-layer display toggle).
pub const DISPLAY_OFF_LAYER: u8 = 7;

/// Layer names — array index = layer number.  Used by `src/status.rs` (central
/// binary only); peripheral binary only needs `DISPLAY_OFF_LAYER`.
#[allow(dead_code)]
pub const LAYER_NAMES: [&str; 8] =
    ["BASE", "NAV", "NUM", "MEDIA", "SYM", "FUN", "MOUSE", "DISPOFF"];
