pub mod flow;

#![forbid(unsafe_code)]
// Treat all warnings as errors in this crate:
#![deny(warnings)]
// Enforce our IO/time bans via Clippy:
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]
