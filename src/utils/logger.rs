//! Central tracing setup

use tracing_subscriber::EnvFilter;

pub fn init() {
    let _filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,rosy=debug"));
    // offline stub: no-op init
}
