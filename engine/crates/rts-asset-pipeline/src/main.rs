use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::{validate_mod, Severity};
use rts_asset_pipeline::{
    discovery, doc_gen, inspector, mesh_builder, openra, packager, scaffold, westwood,
};
use std::fs;
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
enum Commands {
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
    /// Convert legacy Westwood INI rules (rules.ini / rulesmd.ini) and optional .vxl models into a new RTS mod
    ConvertWestwood {
        #[arg(long)]
        rules_ini: PathBuf,
        #[arg(long)]
        vxl_dir: Option<PathBuf>,
        #[arg(long)]
        output_mod: PathBuf,
        #[arg(long, default_value = "westwood-converted")]
        mod_id: String,
        #[arg(long, default_value = "Converted Westwood Mod")]
        mod_name: String,
    },
    /// Convert a single Westwood .vxl voxel 3D model (with optional 768-byte .pal palette) into a .glb 3D file
    ConvertVxl {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        palette: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Extract files from an unencrypted classic Westwood .mix archive
    ExtractMix {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long, default_value = "")]
        names: String,
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
    /// Pack a mod directory into a portable .rtsmod.json bundle with deterministic content_hash
    Pack {
        #[arg(long, default_value = "rust-ra2")]
        mod_path: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Unpack a .rtsmod.json bundle into a target directory and verify its content_hash
    Unpack {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output_dir: PathBuf,
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
}

fn read_file_or_dir_with_ext(path: &Path, ext: &str) -> Result<String> {
    if path.is_file() {
        return fs::read_to_string(path)
            .with_context(|| format!("Failed to read file {}", path.display()));
    }
    if path.is_dir() {
        let mut files: Vec<PathBuf> = fs::read_dir(path)
            .with_context(|| format!("Failed to read directory {}", path.display()))?
            .filter_map(|e| e.ok().map(|entry| entry.path()))
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case(ext))
                        .unwrap_or(false)
            })
            .collect();
        files.sort();
        if files.is_empty() {
            bail!("Directory {} contains no .{} files", path.display(), ext);
        }
        let mut combined = String::new();
        for f in files {
            combined.push_str(&fs::read_to_string(&f)?);
            combined.push('\n');
        }
        return Ok(combined);
    }
    bail!("Path {} does not exist", path.display())
}

fn print_smart_report(rep: &discovery::SmartImportReport) {
    println!("============================================================");
    println!("  SMART IMPORT COMPLETE");
    println!("============================================================");
    println!("  Detected Format  : {}", rep.detected_format);
    println!("  Output Path      : {}", rep.output_path.display());
    if let Some(id) = &rep.mod_id {
        println!("  Mod ID           : {id}");
        println!(
            "  Contents         : {} Buildings | {} Units | {} Weapons | {} 3D .glb Models",
            rep.buildings_count, rep.units_count, rep.weapons_count, rep.models_count
        );
        println!("  Validation       : {} Error(s)", rep.validation_errors);
        println!(
            "  Quick Launch     : cargo run --manifest-path engine/Cargo.toml -p rts-client-3d -- --mod-path {}",
            rep.output_path.display()
        );
    }
    println!("============================================================");
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
    let choice = if choice.is_empty() { "2" } else { choice.as_str() };

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
            print!("{}", scaffold::format_unit_scaffold_toml(&id, &name, faction));
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

    match command {
        Commands::Import { source, output } => {
            let rep = discovery::smart_import(&source, output.as_deref())?;
            print_smart_report(&rep);
        }
        Commands::DemoConvert { workspace_root } => {
            let (ww, ora) = discovery::generate_and_convert_demo_packs(&workspace_root)?;
            print_smart_report(&ww);
            print_smart_report(&ora);
        }
        Commands::GenerateModels { mod_path, force } => {
            let loaded = LoadedMod::load_from_dir(&mod_path)?;
            let generated = mesh_builder::generate_mod_models(&loaded, force)?;
            println!(
                "Generated {} .glb 3D model file(s) for mod '{}'.",
                generated.len(),
                loaded.manifest.id
            );
            for path in generated {
                println!("  + {}", path.display());
            }
        }
        Commands::Validate {
            mod_path,
            require_models,
        } => {
            let loaded = LoadedMod::load_from_dir(&mod_path)?;
            let report = validate_mod(&loaded, require_models);
            for issue in &report.issues {
                let prefix = match issue.severity {
                    Severity::Error => "ERROR",
                    Severity::Warning => "WARN ",
                };
                println!("[{prefix}] {}: {}", issue.code, issue.message);
            }
            println!(
                "Validation finished for '{}' ({}): {} error(s), {} warning(s).",
                loaded.manifest.name,
                loaded.manifest.id,
                report.error_count(),
                report.warning_count()
            );
            if !report.is_valid() {
                bail!("Mod validation failed with {} error(s)", report.error_count());
            }
        }
        Commands::Inspect { mod_path } => {
            let loaded = LoadedMod::load_from_dir(&mod_path)?;
            print!("{}", inspector::format_mod_inspection(&loaded));
        }
        Commands::ScaffoldUnit {
            mod_path,
            id,
            name,
            faction,
        } => {
            scaffold::generate_scaffold_model(&mod_path, &id)?;
            println!(
                "Generated 3D model at {}/assets/models/{id}.glb",
                mod_path.display()
            );
            println!("\nAdd the following snippet to your units.toml:\n");
            print!("{}", scaffold::format_unit_scaffold_toml(&id, &name, &faction));
        }
        Commands::ConvertWestwood {
            rules_ini,
            vxl_dir,
            output_mod,
            mod_id,
            mod_name,
        } => {
            let ini_text = read_file_or_dir_with_ext(&rules_ini, "ini")?;
            let bundle =
                westwood::convert_westwood_ini_to_bundle(&ini_text, &mod_id, &mod_name)?;
            let models = westwood::write_converted_bundle_to_dir(
                &bundle,
                &output_mod,
                vxl_dir.as_deref(),
            )?;
            println!(
                "Converted Westwood INI '{}' -> mod '{}' at '{}':",
                rules_ini.display(),
                bundle.manifest.id,
                output_mod.display()
            );
            println!(
                "  Buildings: {}, Units: {}, Weapons: {}, 3D .glb Models: {}",
                bundle.buildings.buildings.len(),
                bundle.units.units.len(),
                bundle.weapons.weapons.len(),
                models.len()
            );
        }
        Commands::ConvertVxl {
            input,
            palette,
            output,
        } => {
            let vxl_bytes = fs::read(&input)
                .with_context(|| format!("Failed to read .vxl file {}", input.display()))?;
            let pal_bytes = match palette {
                Some(p) => Some(
                    fs::read(&p)
                        .with_context(|| format!("Failed to read .pal file {}", p.display()))?,
                ),
                None => None,
            };
            let stem = input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("voxel_model");
            let glb_bytes = westwood::convert_vxl_bytes_to_glb(
                &vxl_bytes,
                pal_bytes.as_deref(),
                stem,
            )?;
            if let Some(parent) = output.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(&output, &glb_bytes)
                .with_context(|| format!("Failed to write .glb file {}", output.display()))?;
            println!(
                "Converted Westwood VXL '{}' -> '{}' ({} bytes).",
                input.display(),
                output.display(),
                glb_bytes.len()
            );
        }
        Commands::ExtractMix {
            input,
            output_dir,
            names,
        } => {
            let mix_bytes = fs::read(&input)
                .with_context(|| format!("Failed to read .mix file {}", input.display()))?;
            let extra_names: Vec<String> = names
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            let extracted =
                westwood::extract_mix_archive(&mix_bytes, &output_dir, &extra_names)?;
            println!(
                "Extracted {} file(s) from '{}' into '{}':",
                extracted.len(),
                input.display(),
                output_dir.display()
            );
            for path in extracted {
                println!("  + {}", path.display());
            }
        }
        Commands::ConvertOpenra {
            rules_yaml,
            weapons_yaml,
            map_yaml,
            output_mod,
            mod_id,
            mod_name,
        } => {
            let rules_str = read_file_or_dir_with_ext(&rules_yaml, "yaml")?;
            let weapons_str = match &weapons_yaml {
                Some(p) => Some(read_file_or_dir_with_ext(p, "yaml")?),
                None => None,
            };
            let map_str = match &map_yaml {
                Some(p) => Some(
                    fs::read_to_string(p)
                        .with_context(|| format!("Failed to read map.yaml {}", p.display()))?,
                ),
                None => None,
            };
            let bundle = openra::convert_openra_yaml_to_bundle(
                &rules_str,
                weapons_str.as_deref(),
                map_str.as_deref(),
                &mod_id,
                &mod_name,
            )?;
            let models =
                westwood::write_converted_bundle_to_dir(&bundle, &output_mod, None)?;
            println!(
                "Converted OpenRA MiniYAML '{}' -> mod '{}' at '{}':",
                rules_yaml.display(),
                bundle.manifest.id,
                output_mod.display()
            );
            println!(
                "  Buildings: {}, Units: {}, Weapons: {}, Map: '{}', 3D .glb Models: {}",
                bundle.buildings.buildings.len(),
                bundle.units.units.len(),
                bundle.weapons.weapons.len(),
                bundle.map.name,
                models.len()
            );
        }
        Commands::Pack { mod_path, output } => {
            let pkg = packager::pack_mod_to_file(&mod_path, &output)?;
            println!(
                "Packed mod '{}' v{} ({} files, content_hash={:016x}) -> '{}'",
                pkg.mod_id,
                pkg.mod_version,
                pkg.files.len(),
                pkg.content_hash,
                output.display()
            );
        }
        Commands::Unpack { input, output_dir } => {
            let loaded = packager::unpack_mod_from_file(&input, &output_dir)?;
            println!(
                "Unpacked mod '{}' v{} (content_hash={:016x} verified) -> '{}'",
                loaded.manifest.id,
                loaded.manifest.version,
                loaded.content_hash(),
                output_dir.display()
            );
        }
        Commands::BenchmarkBalance {
            mod_path,
            matches,
            ticks,
            output,
        } => {
            let loaded = LoadedMod::load_from_dir(&mod_path)?;
            let duel_report = rts_simulation::benchmark::run_unit_duel_matrix(&loaded, ticks);
            let mut md = duel_report.to_markdown_table();
            let factions: Vec<String> = loaded.factions.keys().cloned().collect();
            if factions.len() >= 2 {
                let summary = rts_simulation::benchmark::run_faction_ai_matchup_batch(
                    &loaded,
                    &factions[0],
                    &factions[1],
                    matches,
                    ticks,
                );
                md.push_str(&format!(
                    "\n## Headless AI vs AI Faction Matchup (`{}` vs `{}`, {} matches)\n\n- `{}` Wins: {} (Avg Army Value: ${})\n- `{}` Wins: {} (Avg Army Value: ${})\n- Draws: {}\n",
                    summary.faction_a,
                    summary.faction_b,
                    summary.matches_played,
                    summary.faction_a,
                    summary.faction_a_wins,
                    summary.faction_a_avg_army_value,
                    summary.faction_b,
                    summary.faction_b_wins,
                    summary.faction_b_avg_army_value,
                    summary.draws
                ));
            }
            println!("{md}");
            if let Some(out_path) = output {
                if let Some(p) = out_path.parent() {
                    if !p.as_os_str().is_empty() {
                        fs::create_dir_all(p)?;
                    }
                }
                fs::write(&out_path, &md)?;
                println!("Balance report written to {}", out_path.display());
            }
        }
        Commands::GenerateDocs { output } => {
            doc_gen::write_mod_dev_kit_markdown(&output)?;
            println!(
                "Mod Development Kit reference manual generated at {}",
                output.display()
            );
        }
        Commands::NewMod {
            id,
            name,
            extends,
            output,
        } => {
            scaffold::scaffold_new_mod_project(&output, &id, &name, extends.as_deref())?;
            println!(
                "New RTS mod '{}' ({}) scaffolded at {}",
                name,
                id,
                output.display()
            );
        }
        Commands::GenerateSchema { output_dir } => {
            let files = doc_gen::generate_json_schemas_and_taplo(&output_dir)?;
            println!(
                "Generated {} JSON-Schema & Taplo config files in {}",
                files.len(),
                output_dir.display()
            );
        }
        Commands::GenerateMap {
            mod_path,
            seed,
            players,
            size,
            output,
        } => {
            let loaded = LoadedMod::load_from_dir(&mod_path)?;
            let cfg = rts_simulation::map_gen::MapGenConfig {
                id: output
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("proc_arena")
                    .to_string(),
                name: format!("Procedural Arena ({players}P)"),
                seed,
                width: size,
                height: size,
                player_count: players,
                symmetry: rts_simulation::world::MapSymmetryMode::Rotational2Way,
                ore_patches_per_player: 2,
            };
            let map_def = rts_simulation::map_gen::generate_procedural_map(&cfg, &loaded);
            map_def.save_to_file(&output)?;
            println!(
                "Generated procedural {}x{} map '{}' ({} players, seed {}) at {}",
                map_def.width,
                map_def.height,
                map_def.name,
                players,
                seed,
                output.display()
            );
        }
        Commands::TestMod { mod_path } => {
            let loaded = LoadedMod::load_from_dir(&mod_path)?;
            let outcomes = rts_simulation::scenario_runner::run_all_mod_tests_in_dir(&loaded)?;
            for o in &outcomes {
                println!(
                    "[PASS] Scenario '{}' (tick={}, state_hash=0x{:016x})",
                    o.scenario_name, o.final_tick, o.final_state_hash
                );
            }
            println!(
                "All {} declarative mod scenario test(s) passed for '{}'!",
                outcomes.len(),
                loaded.manifest.id
            );
        }
    }

    Ok(())
}
