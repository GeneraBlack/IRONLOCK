# Spec: `rts-sdk` (`engine/crates/rts-sdk`)

## Objective
Bereitstellung des daten- und skriptgetriebenen Modding-Fundaments der Engine ([`engine/crates/rts-sdk`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/lib.rs)). Dieses Crate lädt Mods aus dem Dateisystem, validiert alle Referenzen (Fraktionen, Einheiten, Gebäude, Waffen, Superwaffen, Kampagnen, Tech-Tree-Voraussetzungen, `.glb`-3D-Modelle, Karten) und stellt eine einheitliche Sandbox-Skript-Schnittstelle für **Rhai** (`.rhai`) und **Lua** (`.lua`) bereit.

## Tech Stack
- `serde`, `serde_json`, `toml`, `thiserror`, `anyhow`
- `rhai` `1.27` (`Engine::new_raw()` deterministische Sandbox ohne Host-Wall-Clock)
- Deterministisches Lua-Backend ([`lua_runtime.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/scripting/lua_runtime.rs)) mit lexikalischem `local`-Scope, `elseif`, Schleifen-Budgets und exakter `i64`-Arithmetik hinter derselben `ScriptBackend`-Abstraktion

## Commands
```powershell
cargo test --manifest-path engine/Cargo.toml -p rts-sdk
```

## Project Structure
- [`engine/crates/rts-sdk/src/lib.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/lib.rs)
- [`engine/crates/rts-sdk/src/manifest.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/manifest.rs) — `ModManifest` (`mod.toml`)
- [`engine/crates/rts-sdk/src/loader.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/loader.rs) — `LoadedMod` & Pfad-Auflösung (`resolve_mod_dir`)
- [`engine/crates/rts-sdk/src/validator.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/validator.rs) — Cross-Reference- & Tech-Tree-Validator (`validate_mod`)
- [`engine/crates/rts-sdk/src/schema/mod.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/mod.rs) — Modulare TOML-Datenschemata:
  - [`common.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/common.rs) (`VisualDef`, `ArmorType`, `LocomotionType`, `TargetDomain`)
  - [`entities.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/entities.rs) (`FactionDef`, `UnitDef`, `BuildingDef`, `SupportPowerDef`)
  - [`combat_defs.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/combat_defs.rs) (`WeaponDef`, Schadensmultiplikatoren, Projektil-/Strahl-Eigenschaften)
  - [`world_defs.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/world_defs.rs) (`MapDef`, `CampaignDef`, `MissionDef`, `MapTriggerDef`, Spawn- & Erzfelder)
- [`engine/crates/rts-sdk/src/scripting/mod.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/scripting/mod.rs) & [`lua_runtime.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/scripting/lua_runtime.rs) — `ScriptHost`, `ScriptBackend`, `ScriptContext`

## Testing Strategy
- Unit-Tests für TOML-Deserialisierung, Cross-Reference-Validator (inkl. Zykluserkennung im Tech-Tree) sowie deterministische Ausführung von `.rhai`- und `.lua`-Hooks auf einem `ScriptContext`.

## Boundaries
- **Always:** Keine Abhängigkeiten auf Bevy oder Grafik-Bibliotheken; 100 % plattformunabhängig und schnell kompilierbar.
- **Never:** Spielspezifische RA2-Konstanten in `rts-sdk` hinterlegen.
