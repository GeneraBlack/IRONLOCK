use crate::cli_legacy::{
    print_smart_report, run_convert_csf, run_convert_openra, run_convert_vxl, run_convert_westwood,
    run_extract_audio, run_extract_mix,
};
use crate::Commands;
use anyhow::{bail, Context, Result};
use rts_asset_pipeline::{
    cameo_renderer, cook, discovery, doc_gen, glb_optimizer, inspector, mesh_builder, packager,
    scaffold,
};
use rts_sdk::loader::LoadedMod;
use rts_sdk::validator::{validate_mod, Severity};
#[cfg(feature = "sim-tools")]
use std::fs;

pub(crate) fn execute_command(command: Commands) -> Result<()> {
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
            let mut generated = mesh_builder::generate_mod_models(&loaded, force)?;
            let props_dir = loaded.root_dir.join("assets/models/props");
            let props_missing = mesh_builder::PropKind::all()
                .iter()
                .any(|k| !props_dir.join(k.file_name()).exists());
            if force || props_missing {
                let prop_paths = mesh_builder::generate_prop_library(&props_dir)?;
                generated.extend(prop_paths);
            }
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
                bail!(
                    "Mod validation failed with {} error(s)",
                    report.error_count()
                );
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
            print!(
                "{}",
                scaffold::format_unit_scaffold_toml(&id, &name, &faction)
            );
        }
        Commands::ConvertWestwood {
            rules_ini,
            art_ini,
            sound_ini,
            eva_ini,
            vxl_dir,
            output_mod,
            mod_id,
            mod_name,
        } => run_convert_westwood(
            rules_ini, art_ini, sound_ini, eva_ini, vxl_dir, output_mod, mod_id, mod_name,
        )?,
        Commands::ConvertCsf {
            input,
            output,
            lang,
        } => run_convert_csf(input, output, lang)?,
        Commands::ConvertVxl {
            input,
            hva,
            palette,
            output,
        } => run_convert_vxl(input, hva, palette, output)?,
        Commands::ExtractMix {
            input,
            output_dir,
            names,
            key_hex,
        } => run_extract_mix(input, output_dir, names, key_hex)?,
        Commands::ExtractAudio { input, idx, output } => run_extract_audio(input, idx, output)?,
        Commands::ConvertOpenra {
            rules_yaml,
            weapons_yaml,
            map_yaml,
            output_mod,
            mod_id,
            mod_name,
        } => run_convert_openra(
            rules_yaml,
            weapons_yaml,
            map_yaml,
            output_mod,
            mod_id,
            mod_name,
        )?,
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
        Commands::CookStandalone {
            mod_path,
            output_dir,
            title,
            lock_player_mode,
        } => {
            let report = cook::cook_standalone_game(&cook::CookStandaloneOptions {
                mod_path,
                output_dir,
                title,
                lock_player_mode,
            })?;
            println!("============================================================");
            println!("  COOK STANDALONE GAME COMPLETE");
            println!("============================================================");
            println!("  Title            : {}", report.game_title);
            println!(
                "  Mod              : {} v{} (hash={:016x})",
                report.mod_id, report.mod_version, report.content_hash
            );
            println!(
                "  VFS Archive      : {} ({} files, {} bytes compressed / {} bytes raw)",
                report.archive_path.display(),
                report.packed_files_count,
                report.archive_bytes,
                report.uncompressed_bytes
            );
            println!(
                "  Cooked KTX2/Atlas: {} .ktx2 texture(s), {} cameo(s) in atlas, {} dependency archive(s)",
                report.cooked_ktx2_count,
                report.packed_cameos_count,
                report.dependency_archives.len()
            );
            println!(
                "  Bundle Config    : {}",
                report.bundle_config_path.display()
            );
            println!(
                "  Copied Binaries  : {}",
                if report.copied_binaries.is_empty() {
                    "none found in target/ (Play.bat & play.sh generated)".to_string()
                } else {
                    report
                        .copied_binaries
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            );
            println!("============================================================");
        }
        Commands::BenchmarkBalance {
            mod_path,
            matches,
            ticks,
            output,
        } => {
            #[cfg(feature = "sim-tools")]
            {
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
            #[cfg(not(feature = "sim-tools"))]
            {
                let _ = (mod_path, matches, ticks, output);
                bail!("benchmark-balance requires the default 'sim-tools' feature");
            }
        }
        Commands::GenerateDocs { output } => {
            doc_gen::write_mod_dev_kit_markdown(&output)?;
            println!(
                "Mod Development Kit reference manual generated at {}",
                output.display()
            );
        }
        Commands::GenDocs {
            mod_path,
            reference_output,
        } => {
            let report =
                doc_gen::generate_mod_docs_and_schemas(&mod_path, reference_output.as_deref())?;
            println!(
                "Auto-derived {} schemars JSON-Schema files in '{}/schemas', wrote '{}', and updated '{}'.",
                report.schema_files.len(),
                mod_path.display(),
                report.taplo_path.display(),
                report.reference_markdown_path.display()
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
            #[cfg(feature = "sim-tools")]
            {
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
            #[cfg(not(feature = "sim-tools"))]
            {
                let _ = (mod_path, seed, players, size, output);
                bail!("generate-map requires the default 'sim-tools' feature");
            }
        }
        Commands::TestMod { mod_path } => {
            #[cfg(feature = "sim-tools")]
            {
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
            #[cfg(not(feature = "sim-tools"))]
            {
                let _ = mod_path;
                bail!("test-mod requires the default 'sim-tools' feature");
            }
        }
        Commands::OptimizeModels {
            mod_path,
            max_unit_verts,
            max_building_verts,
            lod1_ratio,
            force,
        } => {
            let opts = glb_optimizer::GlbOptimizeOptions {
                max_unit_verts,
                max_building_verts,
                lod1_ratio,
                force,
            };
            let reports = glb_optimizer::optimize_mod_models(&mod_path, &opts)?;
            let mut optimized_count = 0usize;
            let mut skipped_count = 0usize;
            let mut orig_sum = 0usize;
            let mut lod0_sum = 0usize;
            let mut lod1_sum = 0usize;
            for (_path, rep) in &reports {
                if rep.skipped_already_optimized {
                    skipped_count += 1;
                } else {
                    optimized_count += 1;
                    orig_sum += rep.original_vertices;
                    lod0_sum += rep.lod0_vertices;
                    lod1_sum += rep.lod1_vertices;
                }
            }
            println!(
                "Optimized {} .glb model(s) in '{}' ({} already up-to-date): {} orig verts -> {} LOD0 verts + {} LOD1 verts.",
                optimized_count,
                mod_path.display(),
                skipped_count,
                orig_sum,
                lod0_sum,
                lod1_sum
            );
        }
        Commands::ImportAsset {
            source,
            input,
            mod_path,
            id,
            as_building,
            render_cameos,
            optimize_glb,
        } => {
            let src_path = source
                .or(input)
                .context("Source asset path is required for import-asset")?;
            let rep = discovery::import_modern_asset(
                &src_path,
                &mod_path,
                &discovery::ModernAssetImportOptions {
                    asset_id: id,
                    as_building,
                    bake_cameo: render_cameos,
                    optimize_glb,
                },
            )?;
            println!("============================================================");
            println!("  MODERN ASSET IMPORT COMPLETE");
            println!("============================================================");
            println!("  Asset ID         : {}", rep.asset_id);
            println!("  Format           : {}", rep.kind.label());
            println!("  Destination      : {}", rep.destination_path.display());
            if let (Some(bmin), Some(bmax)) = (rep.bounds_min, rep.bounds_max) {
                println!(
                    "  AABB Bounds      : [{:.2}, {:.2}, {:.2}] .. [{:.2}, {:.2}, {:.2}]",
                    bmin[0], bmin[1], bmin[2], bmax[0], bmax[1], bmax[2]
                );
            }
            if let Some(fp) = rep.suggested_footprint {
                println!("  Suggested Tiles  : {}x{}", fp[0], fp[1]);
            }
            if !rep.detected_sockets.is_empty() {
                let names: Vec<&str> = rep
                    .detected_sockets
                    .iter()
                    .map(|(n, _)| n.as_str())
                    .collect();
                println!("  Detected Sockets : {}", names.join(", "));
            }
            if !rep.animation_clips.is_empty() {
                println!("  Animation Clips  : {}", rep.animation_clips.join(", "));
            }
            if let Some(cameo) = &rep.baked_cameo_path {
                println!("  Baked Cameo (96x72): {}", cameo.display());
            }
            println!("------------------------------------------------------------");
            print!("{}", rep.toml_snippet);
            println!("============================================================");
        }
        Commands::RenderCameos {
            mod_path,
            def_id,
            only_missing,
        } => {
            let entries =
                cameo_renderer::render_mod_cameos(&mod_path, def_id.as_deref(), only_missing)?;
            println!(
                "Rendered {} 96x72 RGBA .png cameo(s) in '{}':",
                entries.len(),
                mod_path.display()
            );
            for e in entries {
                println!(
                    "  + {} ({}) -> {}",
                    e.def_id,
                    if e.is_building { "building" } else { "unit" },
                    e.relative_icon_path
                );
            }
        }
    }
    Ok(())
}
