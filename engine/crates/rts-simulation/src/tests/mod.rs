mod audit_fixes;
mod combined_arms;
mod core_mechanics;
mod generals_and_stealth;
mod phase15_16_systems;
mod phase17_20_systems;
mod phase21_24_systems;
mod phase25_26_systems;
mod phase27_28_systems;
mod tactical_and_world;

use rts_sdk::loader::LoadedMod;
use std::path::PathBuf;

pub(super) fn load_ra2_mod() -> LoadedMod {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ra2_dir = manifest_dir.join("../../../rust-ra2");
    LoadedMod::load_from_dir(ra2_dir).expect("rust-ra2 should load cleanly")
}

pub(super) fn load_showcase_mod() -> LoadedMod {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let showcase_dir = manifest_dir.join("../../../mods/sdk-showcase-workers-squads");
    LoadedMod::load_from_dir(showcase_dir)
        .expect("mods/sdk-showcase-workers-squads should load cleanly")
}

pub(super) fn load_showcase2_mod() -> LoadedMod {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let showcase2_dir = manifest_dir.join("../../../mods/sdk-showcase-heroes-ages-mutators");
    LoadedMod::load_from_dir(showcase2_dir)
        .expect("mods/sdk-showcase-heroes-ages-mutators should load cleanly")
}

pub(super) fn load_showcase2_with_turbo_mutator() -> LoadedMod {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let showcase2_dir = manifest_dir.join("../../../mods/sdk-showcase-heroes-ages-mutators");
    let mutator_dir = manifest_dir.join("../../../mods/mutator-turbo-economy");
    LoadedMod::load_mod_with_overlays(&showcase2_dir, &[&mutator_dir])
        .expect("showcase2 + mutator-turbo-economy stack should load cleanly")
}
