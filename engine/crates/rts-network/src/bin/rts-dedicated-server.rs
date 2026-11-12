use anyhow::{Context, Result};
use clap::Parser;
use rts_network::dedicated::{DedicatedMatchConfig, DedicatedMatchHost};
use rts_network::player_identity::PlayerIdentityProfile;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "rts-dedicated-server",
    about = "IRONLOCK Authoritative Headless Dedicated Match & Fleet Server (TCP + UDP + Hybrid Rollback + Staging + Rejoin + Spectator)"
)]
struct DedicatedServerCli {
    /// TCP bind address for handshakes, reconnect snapshots, staging rooms, and spectator streams
    #[arg(long, default_value = "0.0.0.0:7878")]
    bind: String,

    /// UDP bind port for the loss-tolerant K=4 sliding-window tick fast-path (0 = OS-assigned)
    #[arg(long, default_value_t = 7879)]
    udp_port: u16,

    /// Path to the base mod directory or `.rtsmod` archive to load and enforce via content hash
    #[arg(long, default_value = "rust-ra2")]
    mod_path: PathBuf,

    /// Optional stacked overlay/mutator mod paths (repeatable: `--mutator mods/mutator-turbo-economy`)
    #[arg(long = "mutator")]
    mutators: Vec<PathBuf>,

    /// Number of human player slots expected in the match
    #[arg(long, default_value_t = 2)]
    expected_players: u8,

    /// Minimum observer stream delay in simulation ticks (600 ticks = 30s at 20 Hz)
    #[arg(long, default_value_t = 600)]
    spectator_delay_ticks: u64,

    /// Optional Master-Lobby / centralwebserver URL for registration, heartbeats, and match reporting
    #[arg(long)]
    lobby_server: Option<String>,

    /// Public lobby or fleet node display name announced to centralwebserver
    #[arg(long, default_value = "IRONLOCK Dedicated Match")]
    lobby_name: String,

    /// Optional lobby password required for joining players
    #[arg(long)]
    password: Option<String>,

    /// Initial lockstep input delay in simulation ticks (`1..=8`, dynamically adjusted by AdaptiveJitterBuffer)
    #[arg(long, default_value_t = 2)]
    input_delay_ticks: u64,

    /// Optional path to a custom map file (.toml) to stream in 32 KB chunks to joining clients
    #[arg(long)]
    custom_map_file: Option<PathBuf>,

    /// Optional output path to save the authoritative `.replay.json` file upon match completion
    #[arg(long)]
    save_replay: Option<PathBuf>,

    /// Optional tick cap after which the current match finalizes
    #[arg(long)]
    max_ticks: Option<u64>,

    /// Enables a synchronized pre-game `LobbyRoomState` staging room before tick 0
    #[arg(long)]
    staging: bool,

    /// Registers as an allocatable fleet node on `/api/v1/servers/register` for Ranked Automatch
    #[arg(long)]
    fleet: bool,

    /// Optional stable fleet node identifier (`srv-...`)
    #[arg(long)]
    server_id: Option<String>,

    /// Geographic region advertised to fleet matchmaking (`eu-central`, `us-east`, `ap-northeast`, ...)
    #[arg(long, default_value = "eu-central")]
    region: String,

    /// Optional externally reachable `host:port` advertised to connecting WAN clients
    #[arg(long)]
    public_addr: Option<String>,

    /// Cleanly recycles room state after each match finishes instead of exiting the process
    #[arg(long)]
    recycle: bool,

    /// Optional cap on total matches hosted when `--recycle` is enabled
    #[arg(long)]
    max_matches: Option<u64>,

    /// Optional path to a persistent `PlayerIdentityProfile` TOML file used to HMAC-sign Glicko-2 reports
    #[arg(long)]
    identity_file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = DedicatedServerCli::parse();

    let custom_map_bytes = if let Some(ref map_path) = cli.custom_map_file {
        let bytes = fs::read(map_path)
            .with_context(|| format!("Failed to read custom map '{}'", map_path.display()))?;
        let map_id = map_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("custom_map")
            .to_string();
        Some((map_id.clone(), map_id, bytes))
    } else {
        None
    };

    let signing_profile = if let Some(ref id_path) = cli.identity_file {
        Some(PlayerIdentityProfile::load_or_create(
            id_path,
            format!("Dedicated-{}", cli.lobby_name),
        )?)
    } else {
        None
    };

    let config = DedicatedMatchConfig {
        bind_addr: cli.bind,
        udp_port: cli.udp_port,
        expected_players: cli.expected_players.max(1),
        input_delay_ticks: cli.input_delay_ticks.clamp(1, 8),
        spectator_delay_ticks: cli.spectator_delay_ticks.max(1),
        lobby_server: cli.lobby_server,
        lobby_name: cli.lobby_name,
        password: cli.password,
        custom_map_bytes,
        save_replay_path: cli.save_replay,
        max_ticks: cli.max_ticks,
        enable_staging_room: cli.staging,
        initial_rules: None,
        active_mutators: Vec::new(),
        mutator_paths: cli.mutators.clone(),
        fleet_mode: cli.fleet,
        server_id: cli.server_id,
        region: cli.region,
        public_addr: cli.public_addr,
        recycle_on_finish: cli.recycle || cli.fleet,
        max_matches: cli.max_matches,
        signing_profile,
    };

    let mut host = DedicatedMatchHost::start_with_mod_paths(&cli.mod_path, &cli.mutators, config)?;
    let tcp_addr = host.tcp_local_addr()?;
    let udp_addr = host
        .udp_local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|| "disabled".to_string());

    println!("==========================================================================");
    println!("  IRONLOCK // AUTHORITATIVE HEADLESS DEDICATED MATCH & FLEET SERVER");
    println!(
        "  Mod              : {} v{} (hash 0x{:016x})",
        host.loaded_mod.manifest.id,
        host.loaded_mod.manifest.version,
        host.loaded_mod.content_hash()
    );
    if !host.config.active_mutators.is_empty() {
        println!(
            "  Active Mutators  : {}",
            host.config.active_mutators.join(", ")
        );
    }
    println!("  TCP Endpoint     : {tcp_addr}");
    println!("  UDP Fast-Path    : {udp_addr} (K=4 redundant sliding window)");
    println!("  Expected Players : {}", host.config.expected_players);
    println!(
        "  Staging Room     : {}",
        if host.config.enable_staging_room {
            "enabled"
        } else {
            "direct-start"
        }
    );
    println!(
        "  Spectator Delay  : {} ticks ({:.1}s @ 20 Hz)",
        host.config.spectator_delay_ticks,
        host.config.spectator_delay_ticks as f64 / 20.0
    );
    if let Some(ref srv_id) = host.fleet_server_id {
        println!(
            "  Fleet Registry   : {} (registered as {srv_id}, region={})",
            host.config.lobby_server.as_deref().unwrap_or(""),
            host.config.region
        );
    } else if let Some(ref lid) = host.lobby_id {
        println!(
            "  Lobby Registry   : {} (registered as {lid})",
            host.config.lobby_server.as_deref().unwrap_or("")
        );
    }
    println!("==========================================================================");

    host.run_blocking()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_dedicated_server_cli_flags() {
        let cli = DedicatedServerCli::try_parse_from([
            "rts-dedicated-server",
            "--bind",
            "127.0.0.1:7878",
            "--udp-port",
            "7879",
            "--mod-path",
            "mods/sdk-showcase-heroes-ages-mutators",
            "--mutator",
            "mods/mutator-turbo-economy",
            "--expected-players",
            "4",
            "--spectator-delay-ticks",
            "600",
            "--input-delay-ticks",
            "1",
            "--lobby-server",
            "http://127.0.0.1:8080",
            "--lobby-name",
            "Championship Finals",
            "--password",
            "iron-curtain",
            "--staging",
            "--fleet",
            "--region",
            "eu-central",
            "--recycle",
            "--max-matches",
            "5",
        ])
        .expect("CLI flags should parse cleanly");

        assert_eq!(cli.bind, "127.0.0.1:7878");
        assert_eq!(cli.udp_port, 7879);
        assert_eq!(
            cli.mod_path,
            PathBuf::from("mods/sdk-showcase-heroes-ages-mutators")
        );
        assert_eq!(
            cli.mutators,
            vec![PathBuf::from("mods/mutator-turbo-economy")]
        );
        assert_eq!(cli.expected_players, 4);
        assert_eq!(cli.spectator_delay_ticks, 600);
        assert_eq!(cli.input_delay_ticks, 1);
        assert_eq!(cli.lobby_server.as_deref(), Some("http://127.0.0.1:8080"));
        assert_eq!(cli.lobby_name, "Championship Finals");
        assert_eq!(cli.password.as_deref(), Some("iron-curtain"));
        assert!(cli.staging);
        assert!(cli.fleet);
        assert_eq!(cli.region, "eu-central");
        assert!(cli.recycle);
        assert_eq!(cli.max_matches, Some(5));
    }
}
