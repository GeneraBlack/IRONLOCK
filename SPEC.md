# Spec: IRONLOCK // RUST-RTS (`engine/`), `rust-ra2/` Total Conversion & Central Multiplayer Backbone (`centralwebserver/`)

## Objective

Entwicklung und Betrieb einer modernen, daten- und skriptgetriebenen **3D-Echtzeitstrategie-Engine in Rust (`engine/`)**, die ohne Engine-Neukompilierung vollständig über offene Formate (`TOML`, `.gltf`/`.glb`, Skripte in **Rhai** oder **Lua**) gemoddet werden kann, eine vollständige **Asset- & KI-Remastering-Pipeline (`rts-asset-pipeline`)** für klassische Westwood- (`.mix`, `.vxl`, `.shp`, `.ini`, `.wav`, `.bik`) und OpenRA-Assets (`MiniYAML`) mitbringt und über ein **3-Stufen-NAT/UPnP- & Reverse-TCP-Relay-Netzwerk (`rts-network` + `centralwebserver/`)** unter **`https://rust-rts.bls-isp.net/`** weltweiten, desync-freien Online-Multiplayer ermöglicht.

Als Flaggschiff-Referenzprojekt ist im Ordner [`rust-ra2/`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml) eine vollständige **Command & Conquer: Red Alert 2 & Yuri's Revenge Total Conversion** umgesetzt (3 Fraktionen: **Allies**, **Soviet**, **Yuri**, 48 Einheiten, 42 Gebäude, 46 Waffen, 54 spielbare `.toml`-Karten + 298 Original-`.map`-Dateien, 144 `.glb`-3D-Modelle, 404 Audio-Tracks, 138 4K/HD-30fps-Videos und 827 KI-hochskalierte UI-/VFX-Texturen).

### Kern-Architekturprinzipien
1. **Strikte Trennung von Engine und Spielinhalt:** Im Ordner `engine/` existiert keinerlei hartcodiertes Wissen über Fraktionen oder Einheiten. Alle Spielregeln, Tech-Trees, Einheiten, Gebäude, Waffen, Karten und Skripte liegen datengetrieben in `rust-ra2/` bzw. `mods/*`.
2. **Deterministischer 20-Hz-Lockstep-Kern (`rts-simulation`):** Jede Spieler- und KI-Interaktion erzeugt serialisierbare `GameCommand`-Ereignisse. `SimWorld` verarbeitet diese deterministisch (sortierte `BTreeMap`-Container, feste `0.05s`-Ticks) und berechnet in jedem Tick einen 64-Bit FNV-1a `state_hash()` zur mathematischen Desync-Erkennung.
3. **Duale Skript-Unterstützung (Rhai & Lua in `rts-sdk`):** Über das gemeinsame `ScriptBackend`-Trait können Modder ihre Skripte wahlweise in **Rhai** (`.rhai`) oder **Lua** (`.lua`) schreiben (auch gemischt innerhalb derselben Mod).
4. **3D-Asset- & KI-Remaster-Pipeline (`rts-asset-pipeline`):** Konvertiert sowohl prozedurale Archetypen als auch originale Westwood-Voxel (`.vxl` per 3D-Isosurface/Marching-Cubes) und `.shp`-Sprites (3D-Relief & Turntable-Extrusion) in mehrteilige PBR-`.glb`-Modelle; ergänzt durch Vulkan-GPU-beschleunigtes Audio-, Video- (`2880×2160` @ `30 FPS`) und KI-Textur-Upscaling (`Waifu2x` + `Real-ESRGAN`).
5. **3-Stufen-NAT/UPnP-Traversal & Zentrales Multiplayer-Register (`rust-rts.bls-isp.net`):** Kombiniert automatische UPnP-IGD-Portweiterleitung (`nat_upnp.rs`), direkte LAN/WAN-P2P-Verbindungen und einen garantierten **Reverse-TCP-Relay-Tunnel** (`RelayTunnelRoom` auf `rust-rts.bls-isp.net`, Ports `27900–27999`), sodass Hosts auch hinter CGNAT, DS-Lite oder strikten Firewalls immer erreichbar sind.

---

## Capability Map & Modulspezifikationen

Siehe [CAPABILITY_MAP.md](file:///d:/projekt/rust-rts/CAPABILITY_MAP.md) sowie die Detail-Spezifikationen:
- [`SPEC-rts-mod-sdk.md`](file:///d:/projekt/rust-rts/SPEC-rts-mod-sdk.md) — `rts-sdk` (Manifest, TOML-Schemata, Validator, Rhai/Lua-Scripting)
- [`SPEC-rts-sim-core.md`](file:///d:/projekt/rust-rts/SPEC-rts-sim-core.md) — `rts-simulation` (20-Hz-Determinismus, Pathfinding, Combat, Superwaffen, KI)
- [`SPEC-rts-mod-tools.md`](file:///d:/projekt/rust-rts/SPEC-rts-mod-tools.md) — `rts-asset-pipeline` (`.glb`-Generator, Westwood- & OpenRA-Konverter)
- [`SPEC-rts-net.md`](file:///d:/projekt/rust-rts/SPEC-rts-net.md) — `rts-network` & `centralwebserver` (Lockstep, Replay, NAT/UPnP, Reverse-TCP-Relay)
- [`SPEC-rts-client-3d.md`](file:///d:/projekt/rust-rts/SPEC-rts-client-3d.md) — `rts-client` (Bevy 0.15 3D-Client, Audio, VFX, RA2-HUD & Engine Studio)
- [`SPEC-mod-rust-ra2.md`](file:///d:/projekt/rust-rts/SPEC-mod-rust-ra2.md) — `rust-ra2` Total Conversion Mod (Allies, Soviet, Yuri & Remastered Assets)

---

## Tech Stack

- **Engine-Sprache:** Rust (`rustc 1.95+`, Edition 2021)
- **3D-Engine & ECS (`rts-client`):** `bevy` `0.15` (DX12/Vulkan PBR Rendering, `.gltf`/`.glb` Scene Loader, UI, Picking, Gizmos)
- **Datenformate & Serialisierung:** `serde` `1.0`, `toml` `0.8`, `serde_json` `1.0`
- **Duales Mod-Scripting (`rts-sdk`):** `rhai` `1.21` (`.rhai`) + deterministische Lua-Runtime (`.lua`)
- **Zentraler Webserver (`centralwebserver/`):** Node.js 20+ & TypeScript 5 (`http` REST-API, `net` Reverse-TCP Relay-Rooms, Browser-UI)
- **Server-Deployment (`ns8-rust-rts/`):** NethServer 8 (NS8) Rootless Podman Container, Systemd User Units, Traefik + Let's Encrypt, Cockpit Vue/Carbon UI

---

## Commands

```powershell
# Gesamte Rust-Engine kompilieren
cargo build --manifest-path engine/Cargo.toml

# Alle Unit- und Integrationstests über den gesamten Rust-Workspace ausführen
cargo test --manifest-path engine/Cargo.toml --all-targets

# Linter & Formatierungsprüfung
cargo clippy --manifest-path engine/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path engine/Cargo.toml -- --check

# Asset-Pipeline: Mod validieren, inspizieren und 3D-.glb-Modelle generieren
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- validate --mod-path rust-ra2
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- inspect --mod-path rust-ra2
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- generate-models --mod-path rust-ra2

# 3D-RTS-Client starten (Standard: rust-ra2)
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2

# Online-Multiplayer hosten (verbindet sich mit http://rust-rts.bls-isp.net bzw. Fallback http://127.0.0.1:8080)
cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --host 127.0.0.1:7878

# Zentralen Node.js/TypeScript Webserver bauen & testen
cd centralwebserver
npm test
```

---

## Project Structure

```text
d:\projekt\rust-rts\
├── README.md                         → Haupt-Einstiegsdokumentation & Schnellstart
├── CAPABILITY_MAP.md                 → Modul-Übersicht und Abhängigkeitsreihenfolge
├── SPEC.md                           → Technische Hauptspezifikation
├── SPEC-rts-mod-sdk.md               → Modulspezifikation: rts-sdk
├── SPEC-rts-sim-core.md              → Modulspezifikation: rts-simulation
├── SPEC-rts-mod-tools.md             → Modulspezifikation: rts-asset-pipeline
├── SPEC-rts-net.md                   → Modulspezifikation: rts-network & centralwebserver
├── SPEC-rts-client-3d.md             → Modulspezifikation: rts-client
├── SPEC-mod-rust-ra2.md              → Modulspezifikation: rust-ra2 Total Conversion
├── engine/                           → Cargo-Workspace der Rust 3D RTS Engine
│   ├── Cargo.toml                    → Workspace-Manifest (5 Crates)
│   └── crates/
│       ├── rts-sdk/                  → Mod-Loader, TOML-Schemata, Validator, Rhai- & Lua-Scripting
│       ├── rts-asset-pipeline/       → CLI-Tool, .glb-Mesh-Builder, Westwood- & OpenRA-Konverter
│       ├── rts-simulation/           → Deterministische 20-Hz-Simulation, A*, Combat, KI
│       ├── rts-network/              → Lockstep-Transport, Replay, UPnP/NAT, Reverse-TCP-Relay
│       └── rts-client/               → Bevy 0.15 3D-Client, Audio, VFX, RA2-HUD & Engine Studio
├── rust-ra2/                         → Red Alert 2 & Yuri's Revenge Total Conversion Mod
│   ├── mod.toml                      → Mod-Manifest, Einstiegskarte, Kampagnen & Skripte
│   ├── factions/factions.toml        → Allies, Soviet & Yuri (inkl. Länder-Subfraktionen)
│   ├── weapons/weapons.toml          → 46 Waffen (Kanonen, Prisma, Tesla, Raketen, Psycho-Wellen)
│   ├── buildings/buildings.toml      → 42 Gebäude (Basisbau, Verteidigung, Superwaffen, Tech-Gebäude)
│   ├── units/units.toml              → 48 Einheiten (Infanterie, Panzer, Marine, Luftwaffe, Helden)
│   ├── campaigns/campaigns.toml      → Alliierte, Sowjet- & Yuri's-Revenge-Kampagnenmissionen
│   ├── scripts/                      → Rhai- (allied_rules.rhai) & Lua-Skripte (soviet_rules.lua)
│   ├── maps/                         → 54 spielbare .toml-Karten + 298 originale .map-Dateien
│   └── assets/
│       ├── models/                   → 144 mehrteilige .glb 3D-Modelle (aus .vxl, .shp & Archetypen)
│       ├── audio/                    → 404 WAV-Audiodateien (SFX, EVA, Taunts, 26 Musiktracks)
│       ├── videos/                   → 55 4K-30fps Fullscreen-FMVs + 83 HD-30fps Radar-PIP-Videos
│       ├── textures/                 → 827 KI-hochskalierte Texturen (Cameos, 4K-Loadscreens, HUD, VFX)
│       └── data/                     → 22 originale INI/PKT-Regeldateien & 9.592 lokalisierte CSF-Strings
├── mods/                             → Weitere konvertierte Mods (openra-ra, westwood-ra2)
├── centralwebserver/                 → Node.js/TypeScript Master-Lobby-Webserver (rust-rts.bls-isp.net)
└── ns8-rust-rts/                     → Rootless NethServer 8 (NS8) Container-Modul für rust-rts.bls-isp.net
```
