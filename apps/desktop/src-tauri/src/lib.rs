//! C-owned composition root; native adapters are wired here during G2.
//! No microphone, cloud request or database is implicitly started by this stub.
pub use focaus_contracts::{HostServices, StubHost};
pub const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
