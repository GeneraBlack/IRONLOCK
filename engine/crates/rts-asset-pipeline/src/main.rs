mod cli_commands;
mod cli_legacy;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use cli_legacy::print_smart_report;
use rts_asset_pipeline::{discovery, inspector, scaffold};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::validate_mod;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "rts-mod-tools",
    about = "Companion CLI, Smart Auto-Importer & 3D Asset Generator for the Rust RTS Engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Smart Auto-Import: Automatically detects Westwood (.ini, .mix, .vxl) or OpenRA (.yaml) files/folders and converts them
    Import {
        /// Path to any Westwood or OpenRA file or folder (.ini, .mix, .vxl, .yaml, or mod folder)
        source: PathBuf,
        /// Optional output directory or .glb path (defaults to mods/<name>)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate & convert ready-to-play Westwood Classic and OpenRA Classic demo mods into mods/
    DemoConvert {
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,
    },
    /// Generate binary glTF 2.0 (.glb) 3D models for all buildings and units in a mod
    GenerateModels {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long)]
        force: bool,
    },
    /// Validate a mod's manifest, catalogs, tech-tree references, scripts (.rhai/.lua), and 3D models
    Validate {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long, default_value_t = true)]
        require_models: bool,
    },
    /// Inspect factions, buildings, units, DPS stats, and registered script hooks of a mod
    Inspect {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
    },
    /// Scaffold a new unit TOML snippet and generate its initial .glb 3D model
    ScaffoldUnit {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "allies")]
        faction: String,
    },
    /// Convert legacy Westwood INI rules (rules.ini / rulesmd.ini) + optional art.ini, sound.ini, eva.ini and .vxl/.shp assets into a new RTS mod
    ConvertWestwood {
        #[arg(long)]
        rules_ini: PathBuf,
        #[arg(long)]
        art_ini: Option<PathBuf>,
        #[arg(long)]
        sound_ini: Option<PathBuf>,
        #[arg(long)]
        eva_ini: Option<PathBuf>,
        #[arg(long)]
        vxl_dir: Option<PathBuf>,
        #[arg(long)]
        output_mod: PathBuf,
        #[arg(long, default_value = "westwood-converted")]
        mod_id: String,
        #[arg(long, default_value = "Converted Westwood Mod")]
        mod_name: String,
    },
    /// Convert a Westwood .csf binary string table into a clean locales/<lang>.toml file
    ConvertCsf {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        lang: Option<String>,
    },
    /// Convert a single Westwood .vxl voxel 3D model (with optional .hva animation and 768-byte .pal palette) into a .glb 3D file
    ConvertVxl {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        hva: Option<PathBuf>,
        #[arg(long)]
        palette: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Extract files from a classic or Blowfish-encrypted Westwood .mix archive
    ExtractMix {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long, default_value = "")]
        names: String,
        /// Optional hex-encoded 56-byte Blowfish key override for custom encrypted .mix archives
        #[arg(long)]
        key_hex: Option<String>,
    },
    /// Convert Westwood WSADPCM .aud, IMA ADPCM .wav, or audio.bag (+ audio.idx) sound banks into standard 16-bit PCM .wav files
    ExtractAudio {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        idx: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Convert OpenRA MiniYAML rules, weapons, and optional map.yaml into a new RTS mod
    ConvertOpenra {
        #[arg(long)]
        rules_yaml: PathBuf,
        #[arg(long)]
        weapons_yaml: Option<PathBuf>,
        #[arg(long)]
        map_yaml: Option<PathBuf>,
        #[arg(long)]
        output_mod: PathBuf,
        #[arg(long, default_value = "openra-converted")]
        mod_id: String,
        #[arg(long, default_value = "Converted OpenRA Mod")]
        mod_name: String,
    },
    /// Pack a mod directory into a Zlib-compressed RTSMOD03 .rtsmod archive with deterministic content_hash
    Pack {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Unpack an RTSMOD03 / RTSMOD02 .rtsmod archive into a target directory and verify its content_hash
    Unpack {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output_dir: PathBuf,
    },
    /// Cook a validated mod into a self-contained standalone game distribution folder (KTX2 mipmapped textures + cameo atlas + RTSMOD03 archive + game_bundle.toml)
    CookStandalone {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(short, long)]
        output_dir: PathBuf,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, num_args = 0..=1, default_missing_value = "true", default_value_t = true)]
        lock_player_mode: bool,
    },
    /// Run deterministic 1v1 unit duel matrix & AI vs AI faction balance benchmark (with >2.0x cost-efficiency warnings)
    BenchmarkBalance {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long, default_value_t = 2)]
        matches: u32,
        #[arg(long, default_value_t = 200)]
        ticks: u64,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate the Markdown reference manual (docs/MOD_DEV_KIT.md) for all TOML schemas, traits, triggers, and .rhai/.lua APIs
    GenerateDocs {
        #[arg(short, long, default_value = "docs/MOD_DEV_KIT.md")]
        output: PathBuf,
    },
    /// Auto-derive all 14 schemars JSON Schemas (<mod>/schemas/*.schema.json), <mod>/taplo.toml, and docs/MOD_SCHEMA_REFERENCE.md
    GenDocs {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long)]
        reference_output: Option<PathBuf>,
    },
    /// Scaffold a complete new RTS mod project directory (standalone total-conversion or --extends sub-mod)
    NewMod {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        extends: Option<String>,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Generate JSON-Schema files and taplo.toml for IDE TOML autocomplete
    GenerateSchema {
        #[arg(short, long, default_value = "schemas")]
        output_dir: PathBuf,
    },
    /// Generate a deterministic, tournament-balanced procedural skirmish map
    GenerateMap {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 2)]
        players: u8,
        #[arg(long, default_value_t = 64)]
        size: u32,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Execute all declarative mod scenario tests in <mod>/tests/*.toml
    TestMod {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
    },
    /// Simplify high-poly .glb meshes via QEM vertex clustering and embed articulated *_LOD1 nodes
    OptimizeModels {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long, default_value_t = 6_000)]
        max_unit_verts: usize,
        #[arg(long, default_value_t = 12_000)]
        max_building_verts: usize,
        #[arg(long, default_value_t = 0.22)]
        lod1_ratio: f32,
        #[arg(long)]
        force: bool,
    },
    /// Import modern .glb, .png, .ogg, .flac, .wav, or .mp4 assets into a mod workspace with auto AABB/socket/clip inspection
    ImportAsset {
        /// Path to the modern asset file (.glb, .png, .ogg, .flac, .wav, .mp4)
        #[arg(value_name = "SOURCE", required_unless_present = "input")]
        source: Option<PathBuf>,
        /// Optional named alias for the source asset path
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        as_building: bool,
        #[arg(long, num_args = 0..=1, default_missing_value = "true", default_value_t = true)]
        render_cameos: bool,
        #[arg(long, num_args = 0..=1, default_missing_value = "true", default_value_t = true)]
        optimize_glb: bool,
    },
    /// Render deterministic 96x72 RGBA .png cameos directly from .glb 3D models via 2x2 SSAA software rasterization
    RenderCameos {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(long)]
        def_id: Option<String>,
        #[arg(long)]
        only_missing: bool,
    },
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

fn run_interactive_wizard() -> Result<()> {
    println!("====================================================================");
    println!("        RUST 3D RTS ENGINE - INTERACTIVE MOD & IMPORT WIZARD        ");
    println!("====================================================================");
    println!("  [1] Smart Auto-Import (Westwood .ini/.mix/.vxl & OpenRA .yaml)    ");
    println!("  [2] Demo Conversion (Build Westwood & OpenRA Sample Mods)         ");
    println!("  [3] Validate All Available Mods & Check 3D .glb Models            ");
    println!("  [4] Inspect Mod (Units, Buildings, Tech Tree & DPS)               ");
    println!("  [5] Create New Unit + 3D Model (Scaffold)                         ");
    println!("  [0] Exit                                                          ");
    println!("====================================================================");

    let choice = prompt_line("\nEnter selection [1-5, Enter=2]: ")?;
    let choice = if choice.is_empty() {
        "2"
    } else {
        choice.as_str()
    };

    match choice {
        "1" => {
            let src = prompt_line("Path to Westwood/OpenRA file or directory: ")?;
            if src.is_empty() {
                bail!("No source path specified.");
            }
            let out = prompt_line("Optional output directory (Enter = automatic under mods/): ")?;
            let out_opt = if out.is_empty() {
                None
            } else {
                Some(PathBuf::from(out))
            };
            let rep = discovery::smart_import(Path::new(&src), out_opt.as_deref())?;
            print_smart_report(&rep);
        }
        "2" => {
            let (ww, ora) = discovery::generate_and_convert_demo_packs(Path::new("."))?;
            print_smart_report(&ww);
            print_smart_report(&ora);
        }
        "3" => {
            let mods = discovery::discover_available_mods(Path::new("."));
            for m in mods {
                let loaded = LoadedMod::load_from_dir(&m)?;
                let report = validate_mod(&loaded, true);
                println!(
                    "-> Mod '{}' ({}) @ {}: {} error(s), {} warning(s)",
                    loaded.manifest.name,
                    loaded.manifest.id,
                    m.display(),
                    report.error_count(),
                    report.warning_count()
                );
            }
        }
        "4" => {
            let mod_p = prompt_line("Mod path (Enter = rust-ra2): ")?;
            let mod_p = if mod_p.is_empty() {
                "rust-ra2"
            } else {
                mod_p.as_str()
            };
            let loaded = LoadedMod::load_from_dir(mod_p)?;
            print!("{}", inspector::format_mod_inspection(&loaded));
        }
        "5" => {
            let id = prompt_line("Unit ID (e.g. mirage_tank): ")?;
            let name = prompt_line("Display Name (e.g. Mirage Tank): ")?;
            let faction = prompt_line("Faction (allies / soviets, Enter=allies): ")?;
            let faction = if faction.is_empty() {
                "allies"
            } else {
                faction.as_str()
            };
            scaffold::generate_scaffold_model("rust-ra2", &id)?;
            println!("Generated 3D model: rust-ra2/assets/models/{id}.glb\n");
            print!(
                "{}",
                scaffold::format_unit_scaffold_toml(&id, &name, faction)
            );
        }
        _ => {
            println!("Exited.");
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        return run_interactive_wizard();
    };

    cli_commands::execute_command(command)
}
