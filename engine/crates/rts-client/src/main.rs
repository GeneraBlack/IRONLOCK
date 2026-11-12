//! Binary entrypoint for `rts-client` delegating to the modular `rts_client` library (`RtsEngineClientPlugins`).

use anyhow::Result;
use clap::Parser;
use rts_client::{run_rts_client_from_cli, ClientCli};

fn main() -> Result<()> {
    let cli = ClientCli::parse();
    run_rts_client_from_cli(cli)
}
