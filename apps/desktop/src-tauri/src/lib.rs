//! C-owned composition root; native adapters are wired here during G2.
//! No microphone, cloud request or database is implicitly started by this stub.
pub use focaus_contracts::{HostServices, StubHost};
pub const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");

pub mod wire;

#[cfg(feature = "native-shell")]
pub mod shell;

// C composition root: the native modules own durable state and are always
// compiled in the desktop library so the same host is exercised by tests and
// the macOS shell. They never start device capture during construction.
pub mod audio;
pub mod capture;
pub mod ipc;
pub mod library;
pub mod lifecycle;
pub mod search;
pub mod storage;
pub mod tasks;
pub mod visualizations;
