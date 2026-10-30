//! Typed application settings with defaults and overrides.
//!
//! This crate never reads the process environment itself: the composition root
//! passes a lookup function, which keeps loading pure and testable.
