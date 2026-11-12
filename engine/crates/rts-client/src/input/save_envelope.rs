use super::ClientInteractionState;
use rts_sdk::schema::{SaveGameEnvelope, SaveGameHeader};
use rts_simulation::world::SimWorld;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Saves a Zlib-compressed `SaveGameEnvelope` (`.rtssav`) with rich `SaveGameHeader` metadata.
pub fn save_world_envelope_to_path(
    path: &Path,
    save_title: &str,
    world: &SimWorld,
    loaded: &rts_sdk::loader::LoadedMod,
    interaction: &ClientInteractionState,
    camera_state: Option<([f32; 3], f32, f32)>,
) -> anyhow::Result<()> {
    let now_utc = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let pid = interaction.local_player_id;
    let credits = world.players.get(&pid).map(|e| e.credits).unwrap_or(0);
    let (cam_focus, cam_dist, cam_yaw) = camera_state.unwrap_or(([24.0, 0.0, 24.0], 23.5, 0.0));
    let header = SaveGameHeader {
        save_title: save_title.to_string(),
        saved_at_utc: now_utc,
        mod_id: loaded.manifest.id.clone(),
        mod_version: loaded.manifest.version.clone(),
        mod_content_hash: loaded.content_hash(),
        map_id: if interaction.active_map_id.is_empty() {
            loaded.entry_map.id.clone()
        } else {
            interaction.active_map_id.clone()
        },
        map_name: loaded.entry_map.name.clone(),
        is_campaign: interaction.active_campaign_id.is_some(),
        campaign_id: interaction.active_campaign_id.clone(),
        mission_id: interaction.active_mission_id.clone(),
        local_player_id: pid,
        play_seconds: world.tick / 20,
        player_credits: credits,
        camera_focus: cam_focus,
        camera_distance: cam_dist,
        camera_yaw: cam_yaw,
        ..Default::default()
    };
    let stage_path = path.with_extension("stage_tmp.json");
    world.save_to_file(&stage_path)?;
    let world_json_res = std::fs::read_to_string(&stage_path);
    let _ = std::fs::remove_file(&stage_path);
    let world_json = world_json_res?;
    let envelope = SaveGameEnvelope::new(header, &world_json, None)?;
    envelope.save_to_file(path)?;
    Ok(())
}

/// Loads either a framed `.rtssav` `SaveGameEnvelope` or legacy uncompressed `.json` `SimWorld`.
pub fn load_world_envelope_from_path(
    path: &Path,
) -> anyhow::Result<(SimWorld, Option<SaveGameHeader>)> {
    let decode_world_from_json = |base_path: &Path, json: &str| -> anyhow::Result<SimWorld> {
        let stage_path = base_path.with_extension("load_tmp.json");
        if let Some(parent) = stage_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(&stage_path, json)?;
        let res = SimWorld::load_from_file(&stage_path);
        let _ = std::fs::remove_file(&stage_path);
        let mut world = res?;
        world.grid.rebuild_height_cache();
        world.grid.rebuild_clearance_map();
        Ok(world)
    };
    let is_rtssav = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("rtssav"));
    if is_rtssav {
        let env = SaveGameEnvelope::load_from_file(path)?;
        let json = env.decompress_world_json()?;
        let world = decode_world_from_json(path, &json)?;
        return Ok((world, Some(env.header)));
    }
    if let Ok(env) = SaveGameEnvelope::load_from_file(path) {
        let json = env.decompress_world_json()?;
        let world = decode_world_from_json(path, &json)?;
        return Ok((world, Some(env.header)));
    }
    let world = SimWorld::load_from_file(path)?;
    Ok((world, None))
}

/// Loads either a framed `.rtssav` `SaveGameEnvelope` or legacy uncompressed `.json` `SimWorld`,
/// returning `(world, optional_header, mod_compatible)`.
pub fn load_world_envelope_checked_from_path(
    path: &Path,
    loaded: &rts_sdk::loader::LoadedMod,
) -> anyhow::Result<(SimWorld, Option<SaveGameHeader>, bool)> {
    let is_rtssav = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("rtssav"));
    if is_rtssav {
        let env = SaveGameEnvelope::load_from_file(path)?;
        let compat = env
            .header
            .is_mod_compatible(&loaded.manifest.id, loaded.content_hash());
        let (world, hdr) = load_world_envelope_from_path(path)?;
        return Ok((world, hdr, compat));
    }
    if let Ok(env) = SaveGameEnvelope::load_from_file(path) {
        let compat = env
            .header
            .is_mod_compatible(&loaded.manifest.id, loaded.content_hash());
        let (world, hdr) = load_world_envelope_from_path(path)?;
        return Ok((world, hdr, compat));
    }
    let (world, hdr) = load_world_envelope_from_path(path)?;
    Ok((world, hdr, true))
}
