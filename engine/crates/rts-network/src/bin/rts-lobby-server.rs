use anyhow::Result;
use clap::Parser;
use rts_network::lobby_registry::LobbyRegistryServer;

#[derive(Parser, Debug)]
#[command(
    name = "rts-lobby-server",
    about = "Online Multiplayer Master-Lobby-Webserver (REST API & Westwood Online HTML5 Dashboard)"
)]
struct LobbyServerCli {
    /// TCP HTTP bind address for the master lobby webserver
    #[arg(long, default_value = "0.0.0.0:8080")]
    bind: String,

    /// Seconds before an inactive lobby without heartbeat is automatically pruned
    #[arg(long, default_value_t = 45)]
    ttl_secs: u64,
}

fn main() -> Result<()> {
    let cli = LobbyServerCli::parse();
    let mut server = LobbyRegistryServer::bind(&cli.bind, cli.ttl_secs)?;
    let local_addr = server.local_addr()?;
    println!("==========================================================================");
    println!("  COMMAND & CONQUER: RED ALERT 2 — ONLINE MASTER LOBBY WEBSERVER");
    println!("  Web Dashboard (Browser): http://{local_addr}/");
    println!("  REST API Endpoints     : http://{local_addr}/api/lobbies");
    println!("  Heartbeat TTL          : {} seconds", cli.ttl_secs);
    println!("==========================================================================");
    server.run_blocking()
}
