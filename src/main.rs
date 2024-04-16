//! Main [`exsb`] program entry point.
//!
//! Simply delegates to the exsb [`Cli`] wrapper.

use exsb::Cli;

/// Main program entry point.
#[tokio::main]
async fn main() -> exsb::Result<()> {
    Cli::execute().await
}
