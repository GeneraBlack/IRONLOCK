# Spec: `mod-rust-ra2` (`rust-ra2/`) — Command & Conquer: Red Alert 2 & Yuri's Revenge Total Conversion

## Objective
Vollständige Referenz- und Flaggschiff-Total-Conversion-Mod von *Command & Conquer: Red Alert 2 & Yuri's Revenge* im Verzeichnis [`rust-ra2/`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml), die alle Fähigkeiten der Engine demonstriert: **3 asymmetrische Fraktionen** (**Allies**, **Soviet**, **Yuri** inkl. Länder-Spezialeinheiten), **48 Einheiten**, **42 Gebäude**, **46 Waffen**, Superwaffen, **3 Kampagnen**, **52 spielbare `.toml`-Karten**, deterministische Skripte in **Rhai** und **Lua** sowie ein komplett aufbereitetes 3D-, Audio-, 4K-Video- und UI/VFX-Textur-Asset-Paket.

## Umfang & Remastered Assets in `rust-ra2/`

| Kategorie | Pfad | Umfang & Technische Spezifikation |
| :--- | :--- | :--- |
| **Manifest & Regeln** | [`rust-ra2/mod.toml`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml), `factions/`, `units/`, `buildings/`, `weapons/`, `campaigns/` | **3 Fraktionen** (`allies`, `soviets`, `yuri`), **48 Einheiten**, **42 Gebäude**, **46 Waffen**, **3 Kampagnen** |
| **Duale Skripte** | [`rust-ra2/scripts/allied_rules.rhai`](file:///d:/projekt/rust-rts/rust-ra2/scripts/allied_rules.rhai), [`soviet_rules.lua`](file:///d:/projekt/rust-rts/rust-ra2/scripts/soviet_rules.lua) | Deterministische Rhai- & Lua-Hooks für Spezialfähigkeiten (GI Deploy, Chrono-Boost, Tesla/Rhino Overcharge, Yuri Mind-Control) & Kampagnen-Direktoren |
| **Karten (Maps)** | [`rust-ra2/maps/`](file:///d:/projekt/rust-rts/rust-ra2/maps) | **52 spielbare 3D-`.toml`-Karten** (40 Kampagnenmissionen & 12 Skirmish/Multiplayer-Karten mit authentischen Wasser-, Brücken- und Höhenzonen) |
| **3D-Modelle (`.glb`)** | [`rust-ra2/assets/models/`](file:///d:/projekt/rust-rts/rust-ra2/assets/models) | Binary-glTF-2.0-Modelle (`.glb`) aus mehrteiligen `.vxl`/`.hva`-Voxel-Isosurfaces und prozeduralen Rigs mit benannten Sub-Nodes (`Hull`, `Turret`, `Barrel`, `Rotor`) sowie `.shp`-3D-Reliefs |
| **Audio & Musik (`.wav`)** | [`rust-ra2/assets/audio/`](file:///d:/projekt/rust-rts/rust-ra2/assets/audio) | **404 Audiodateien**: Core-Cues, Einheiten-/Kampf-SFX (`sfx/`), EVA-Sprachansagen (`eva/`), Länder-Taunts (`taunts/`) & Musiktracks (`music/`) |
| **4K- & HD-Videos (`.mp4`)** | [`rust-ra2/assets/videos/`](file:///d:/projekt/rust-rts/rust-ra2/assets/videos) | **138 deinterlacte 30-FPS-Videos**: **55 Fullscreen-FMVs** in **4K (`2880×2160`)** (`fmv/`) & **83 Radar-PIP-Videos** in **`1120×880`** (`radar_pip/`) |
| **UI- & VFX-Texturen (`.png`)** | [`rust-ra2/assets/ui/`](file:///d:/projekt/rust-rts/rust-ra2/assets/ui), [`assets/vfx/`](file:///d:/projekt/rust-rts/rust-ra2/assets/vfx) | Vollständiger Cameo-Satz (`cameos/`), Fraktions-HUD-Skins (`hud/`), Ladebildschirme (`loading/`), animierte Cursor (`cursors/`) & VFX-Flipbooks (`vfx_catalog.json`) |
| **Lokalisierung & INI-Daten** | [`rust-ra2/assets/data/`](file:///d:/projekt/rust-rts/rust-ra2/assets/data) | **22 originale INI/PKT-Konfigurationsdateien** (`ini/`) + **9.592 lokalisierte Strings** in `strings/ra2md_strings.json` |

## Commands
```powershell
# rust-ra2 vollständig validieren
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- validate --mod-path rust-ra2

# Tech-Tree & Einheiten-DPS von rust-ra2 inspizieren
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- inspect --mod-path rust-ra2

# rust-ra2 im 3D-Client starten
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2
```
