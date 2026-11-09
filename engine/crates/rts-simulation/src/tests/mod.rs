mod combined_arms;
mod core_mechanics;
mod generals_and_stealth;
mod tactical_and_world;

use rts_sdk::loader::LoadedMod;
use std::path::PathBuf;

pub(super) fn load_ra2_mod() -> LoadedMod {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ra2_dir = manifest_dir.join("../../../rust-ra2");
    LoadedMod::load_from_dir(ra2_dir).expect("rust-ra2 should load cleanly")
}
