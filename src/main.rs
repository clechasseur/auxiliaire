//! Main [`auxiliaire`] program entry point.
//!
//! Simply delegates to the auxiliaire [`Cli`] wrapper.

use auxiliaire::Cli;

/// Main program entry point.
#[tokio::main]
async fn main() -> auxiliaire::Result<()> {
    Cli::execute().await
}
