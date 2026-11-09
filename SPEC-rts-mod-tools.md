# Spec: `rts-asset-pipeline` (`engine/crates/rts-asset-pipeline`)

## Objective
Umfassendes CLI-Werkzeug und Bibliotheks-Crate ([`engine/crates/rts-asset-pipeline`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/lib.rs)) für Mod-Entwickler zum Validieren von Mods (`validate`), Inspizieren von Balancing/Tech-Trees (`inspect`), prozeduralen Generieren mehrteiliger 3D-`.glb`-Modelle (`generate-models`), Anlegen neuer Einheiten-Vorlagen (`scaffold-unit`) sowie zum vollautomatischen Konvertieren klassischer **Westwood**-Archive und -Formate (`.mix`, `.vxl` Voxel-Isosurfaces, `.shp` 3D-Relief & Turntable-Extrusion, `.ini`, `.pal`, `.wav`) und **OpenRA**-Daten (MiniYAML `rules.yaml`, `weapons.yaml`, `map.yaml`) in die modernen Formate der Engine.

## Tech Stack
- `rts-sdk`, `clap`, `serde_json`, `toml`, `anyhow`

## Commands
```powershell
# 3D-.glb-Modelle für alle Einheiten und Gebäude einer Mod generieren
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- generate-models --mod-path rust-ra2

# Mod auf Schema-, Referenz-, Asset- und Skriptfehler (Rhai + Lua) prüfen
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- validate --mod-path rust-ra2

# Tech-Tree, Fraktionen und DPS-Statistiken inspizieren
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- inspect --mod-path rust-ra2

# Komplette Westwood-Installation / .mix-Archive konvertieren
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- convert-westwood --rules-ini path/to/rulesmd.ini --vxl-dir path/to/vxls --output-mod mods/westwood-ra2

# Einzelnes .vxl-Voxelmodell mit Palette in ein 3D-.glb-Mesh (Marching Cubes / Smooth Isosurface) konvertieren
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- convert-vxl --input path/to/mtnk.vxl --palette path/to/unittem.pal --output path/to/mtnk.glb

# Westwood .mix-Archiv entpacken
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- extract-mix --input path/to/ra2.mix --output-dir extracted_mix

# OpenRA MiniYAML-Mod konvertieren
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- convert-openra --rules-yaml path/to/rules --weapons-yaml path/to/weapons --map-yaml path/to/map.yaml --output-mod mods/openra-ra
```

## Project Structure
- [`engine/crates/rts-asset-pipeline/src/main.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/main.rs) & [`lib.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/lib.rs)
- [`engine/crates/rts-asset-pipeline/src/inspector.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/inspector.rs) — Tech-Tree- & DPS-Analyse
- [`engine/crates/rts-asset-pipeline/src/scaffold.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/scaffold.rs) — Einheiten-/Gebäude-Scaffolding
- [`engine/crates/rts-asset-pipeline/src/mesh_builder/`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/mesh_builder/mod.rs) — Binary-glTF-2.0-Generator (`primitives.rs`, `components.rs`, `units.rs`, `buildings.rs`) mit 4 PBR-Materialien (Primary, Secondary, Dark Metal, Emissive Glow) und benannten Sub-Nodes (`Hull`, `Turret`, `Barrel`, `Rotor`)
- [`engine/crates/rts-asset-pipeline/src/westwood/`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/mod.rs) — Westwood-Konverter:
  - [`mix_and_audio.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/mix_and_audio.rs) (`.mix`-Archiv-Reader & IMA-ADPCM `.wav`-Decoder)
  - [`voxel_vxl.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/voxel_vxl.rs), [`voxel_meshing.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/voxel_meshing.rs), [`voxel_isosurface.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/voxel_isosurface.rs) (`.vxl` Voxel-Parser & glatte 3D-Isosurface-Rekonstruktion)
  - [`sprite_shp.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/sprite_shp.rs), [`sprite_relief.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/sprite_relief.rs), [`sprite_turntable.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/sprite_turntable.rs) (`.shp` 2.5D/3D-Gebäude-Relief & 8-Ansichten-Visual-Hull-Rekonstruktion für Infanterie)
  - [`ini_parser.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/ini_parser.rs), [`rules_converter.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/rules_converter.rs), [`bundle_writer.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/westwood/bundle_writer.rs)
- [`engine/crates/rts-asset-pipeline/src/openra/`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/openra/mod.rs) — OpenRA MiniYAML- & Map-Konverter (`miniyaml.rs`, `map_reader.rs`, `converter.rs`)
- [`engine/crates/rts-asset-pipeline/src/discovery/`](file:///d:/projekt/rust-rts/engine/crates/rts-asset-pipeline/src/discovery/mod.rs) — Auto-Discovery lokaler Spielinstallationen & Demo-Packs (`demo_packs.rs`)
