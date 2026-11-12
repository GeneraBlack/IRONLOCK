use super::spawn_body_text;
use crate::input::{discover_compatible_mutators, DiscoveredMutatorEntry, UiAssetCache};
use crate::ui::studio_layout::spawn_styled_button;
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_network::lobby_room::LobbyRoomState;
use rts_sdk::loader::{LoadedMod, ModLayerSource};
use rts_sdk::manifest::ModManifest;
use std::path::{Path, PathBuf};

fn resolve_workspace_root_for_mutators(loaded: &LoadedMod) -> PathBuf {
    loaded
        .root_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .or_else(|| {
            LoadedMod::resolve_mod_dir("rust-ra2")
                .parent()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn collect_compatible_and_cross_mod_mutators(
    ui_cache: &mut UiAssetCache,
    loaded: &LoadedMod,
) -> (Vec<DiscoveredMutatorEntry>, Vec<(String, String)>) {
    let base_mod_id = loaded
        .manifest
        .extends
        .as_deref()
        .unwrap_or(&loaded.manifest.id);
    let mut compat = ui_cache.refresh_compatible_mutators(base_mod_id).to_vec();
    let ws_root = resolve_workspace_root_for_mutators(loaded);
    if compat.is_empty() {
        compat = discover_compatible_mutators(&ws_root, base_mod_id);
    }
    let mut cross_mod = Vec::new();
    if compat.is_empty() {
        for path in rts_asset_pipeline::discovery::discover_available_mods(&ws_root) {
            let manifest_opt: Option<ModManifest> = ModLayerSource::open_path(path)
                .ok()
                .and_then(|s| s.read_to_string("mod.toml").ok())
                .and_then(|raw| toml::from_str(&raw).ok());
            if let Some(m) = manifest_opt {
                if m.id == base_mod_id {
                    continue;
                }
                if m.id.starts_with("mutator-") || !m.depends_on.is_empty() {
                    let req = m
                        .depends_on
                        .first()
                        .map(|d| d.id.clone())
                        .or_else(|| m.extends.clone())
                        .unwrap_or_else(|| "any".to_string());
                    cross_mod.push((m.name, req));
                }
            }
        }
    }
    (compat, cross_mod)
}

/// Renders the Multiplayer Staging Room `Hybrid Rollback (1-Tick)` netcode toggle and compatible
/// Mutator / Overlay checkboxes (`Tasks 246-LobbyUI & 250-Client`).
pub fn spawn_staging_mutators_and_rollback(
    col: &mut ChildBuilder,
    ui_cache: &mut UiAssetCache,
    loaded: &LoadedMod,
    room: &LobbyRoomState,
    is_host: bool,
) {
    let rb_on = room.rules.use_hybrid_rollback;
    let rb_lbl = if rb_on {
        "NETCODE: HYBRID ROLLBACK (1-TICK INSTANT)"
    } else {
        "NETCODE: CLASSIC LOCKSTEP (2-TICK DELAY)"
    };
    if is_host {
        spawn_styled_button(
            col,
            rb_lbl,
            SidebarAction::StagingToggleHybridRollback,
            if rb_on {
                Color::srgb(0.18, 0.44, 0.34)
            } else {
                Color::srgb(0.26, 0.28, 0.38)
            },
            Val::Percent(100.0),
        );
    } else {
        spawn_body_text(col, rb_lbl, Color::srgb(0.68, 0.92, 0.84));
    }

    let (mutators, cross_mod) = collect_compatible_and_cross_mod_mutators(ui_cache, loaded);
    spawn_body_text(
        col,
        format!(
            "Mutator Overlays ({} active):",
            room.rules.active_mutators.len()
        ),
        Color::srgb(0.98, 0.86, 0.44),
    );
    if !mutators.is_empty() {
        for mut_entry in mutators.iter().take(3) {
            let enabled = room
                .rules
                .active_mutators
                .iter()
                .any(|m| m == &mut_entry.id);
            let mark = if enabled { "[X]" } else { "[ ]" };
            if is_host {
                spawn_styled_button(
                    col,
                    &format!("{mark} {}", mut_entry.name),
                    SidebarAction::StagingToggleMutator(mut_entry.id.clone()),
                    if enabled {
                        Color::srgb(0.38, 0.32, 0.14)
                    } else {
                        Color::srgb(0.16, 0.20, 0.28)
                    },
                    Val::Percent(100.0),
                );
            } else {
                spawn_body_text(
                    col,
                    format!("{mark} {}", mut_entry.name),
                    if enabled {
                        Color::srgb(0.98, 0.88, 0.48)
                    } else {
                        Color::srgb(0.58, 0.66, 0.76)
                    },
                );
            }
        }
    } else if let Some((cross_name, req_mod)) = cross_mod.first() {
        spawn_body_text(
            col,
            format!("[—] {cross_name} ({req_mod})"),
            Color::srgb(0.62, 0.72, 0.84),
        );
    } else {
        spawn_body_text(
            col,
            "[—] Base ruleset active (author overlays in Workbench)",
            Color::srgb(0.62, 0.72, 0.84),
        );
    }
}
