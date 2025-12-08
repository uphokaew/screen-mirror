pub mod assets;
pub mod audio;
/// Ultra-low latency screen mirroring application library
///
/// This library provides the core functionality for high-performance screen
/// mirroring from Android to PC with support for both wired (USB/TCP) and
/// wireless (WiFi/QUIC) connections.
pub mod config;

pub mod network;
pub mod platform;
pub mod server;
pub mod sync;
pub mod ui;
pub mod video;

pub use config::Config;
pub use network::{Connection, ConnectionMode};

/// Result type for the application
pub type Result<T> = anyhow::Result<T>;
