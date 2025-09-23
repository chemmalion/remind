#![forbid(unsafe_code)]
// Treat all warnings as errors in this crate:
#![deny(warnings)]
// Enforce our IO/time bans via Clippy:
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

pub mod flow;

pub use flow::{
    CellRef, Command, ConfigDiscovery, EffId, Effect, EmailField, EmailSearchRequest, Event,
    GoogleSheetRequest, ReminderFlow, TelegramRequest, CONFIG_DISCOVERY, CONFIG_ENV_VAR,
    DEFAULT_CONFIG_PATHS,
};
