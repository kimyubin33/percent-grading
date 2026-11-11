pub mod benchmark;
pub mod benchmark_gate;
pub mod benchmark_runner;
pub mod contract;
pub mod discovery;
pub mod evidence;
pub mod evidence_runner;
pub mod grading;
pub mod proof;
pub mod providers;
pub mod relspec;
pub mod research;
pub mod revision;
pub mod runner;
pub mod server;
pub mod service;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
