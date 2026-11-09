# Spec: `mod-rust-ra2` (`rust-ra2/`) — Command & Conquer: Red Alert 2 & Yuri's Revenge Total Conversion

## Objective
Vollständige Referenz- und Flaggschiff-Total-Conversion-Mod von *Command & Conquer: Red Alert 2 & Yuri's Revenge* im Verzeichnis [`rust-ra2/`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml), die alle Fähigkeiten der Engine demonstriert: **3 asymmetrische Fraktionen** (**Allies**, **Soviet**, **Yuri** inkl. Länder-Spezialeinheiten), **48 Einheiten**, **42 Gebäude**, **46 Waffen**, Superwaffen, **3 Kampagnen**, **54 spielbare `.toml`-Karten** (+ 298 originale `.map`-Dateien), Skripte in **Rhai** und **Lua** sowie ein komplett auf der **AMD Radeon RX 7900 XTX (`vulkan=vk:0`)** KI-remastertes 3D-, Audio-, 4K-Video- und Textur-Asset-Paket.

## Umfang & Remastered Assets in `rust-ra2/`

| Kategorie | Pfad | Umfang & Technische Spezifikation |
| :--- | :--- | :--- |
| **Manifest & Regeln** | [`rust-ra2/mod.toml`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml), `factions/`, `units/`, `buildings/`, `weapons/`, `campaigns/` | **3 Fraktionen** (`allies`, `soviets`, `yuri`), **48 Einheiten**, **42 Gebäude**, **46 Waffen**, **3 Kampagnen** |
| **Duale Skripte** | [`rust-ra2/scripts/allied_rules.rhai`](file:///d:/projekt/rust-rts/rust-ra2/scripts/allied_rules.rhai), [`soviet_rules.lua`](file:///d:/projekt/rust-rts/rust-ra2/scripts/soviet_rules.lua) | Rhai- & Lua-Hooks für Spezialfähigkeiten (GI Deploy, Chrono-Boost, Tesla/Rhino Overcharge, Yuri Mind-Control) |
| **Karten (Maps)** | [`rust-ra2/maps/`](file:///d:/projekt/rust-rts/rust-ra2/maps) | **54 spielbare 3D-`.toml`-Karten** (Kampagne & Multiplayer) + **298 originale `.map`-Dateien** in `maps/original/` |
| **3D-Modelle (`.glb`)** | [`rust-ra2/assets/models/`](file:///d:/projekt/rust-rts/rust-ra2/assets/models) | **144 Binary-glTF-2.0-Modelle (`.glb`)** aus echten `.vxl`-Voxel-Isosurfaces (Fahrzeuge/Flugzeuge/Schiffe mit separatem `Turret`/`Barrel`) und `.shp`-3D-Reliefs (Gebäude & Infanterie) |
| **Audio & Musik (`.wav`)** | [`rust-ra2/assets/audio/`](file:///d:/projekt/rust-rts/rust-ra2/assets/audio) | **404 Audiodateien**: `6` Core-Cues, `100` Einheiten-/Kampf-SFX (`sfx/`), `192` EVA-Sprachansagen (`eva/`), `80` Länder-Taunts (`taunts/`), `26` Musiktracks in voller Länge (`music/`) |
| **4K- & HD-Videos (`.mp4`)** | [`rust-ra2/assets/videos/`](file:///d:/projekt/rust-rts/rust-ra2/assets/videos) | **138 deinterlacte 30-FPS-Videos** (`bwdif` + Vulkan Deblock/CAS/lanczos + `h264_amf`): **55 Fullscreen-FMVs** in **4K (`2880×2160`)** (`fmv/`) & **83 Radar-PIP-Videos** in **`1120×880`** (`radar_pip/`) |
| **KI-Remastered Texturen (`.png`)** | [`rust-ra2/assets/textures/`](file:///d:/projekt/rust-rts/rust-ra2/assets/textures) | **827 Neural-AI-Texturen** (`Waifu2x CUNet` + `Real-ESRGAN x4plus` auf RX 7900 XTX): **207 UI-Cameos** (`8×` `480×384`), **51 Ladebildschirme** (`4×` 4K+ `3200×2400`), **36 HUD-Panels** (`4×`), **517 Maus-Cursor** (`4×` `2640×7568`), **16 VFX-Flipbooks** (`2×`–`4×`) |
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
