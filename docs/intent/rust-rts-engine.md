# Statement of Intent: IRONLOCK // RUST-RTS Engine, `rust-ra2` Total Conversion & Central Multiplayer Backbone (`rust-rts.bls-isp.net`)

- **Outcome:** Eine moderne, daten- und skriptgetriebene 3D-RTS-Engine in Rust (`engine/` mit `rts-sdk`, `rts-asset-pipeline`, `rts-simulation`, `rts-network`, `rts-client`) inklusive vollständiger Westwood-/OpenRA-Konvertierungspipeline, `rust-ra2/` als KI-remasterter *Red Alert 2 & Yuri's Revenge* Total Conversion sowie `centralwebserver/` & `ns8-rust-rts/` als zentralem Online-Multiplayer- und NAT-Relay-Backbone unter `https://rust-rts.bls-isp.net/`.
- **User:** Spieler, Modder und Engine-Entwickler, die klassische und neue RTS-Spiele in moderner 3D-Grafik mit deterministischem 20-Hz-Lockstep-Multiplayer (inkl. CGNAT/DS-Lite-Traversal) erleben und ohne Engine-Neukompilierung über TOML, `.glb` und Rhai/Lua modden wollen.
- **Why now:** Klassische 2D-Engines leiden unter veralteten Grafik-/Netzwerk-Architekturen und fehlendem NAT-Traversal; **IRONLOCK // RUST-RTS** verbindet eine kompromisslos deterministische Rust/Bevy-3D-Architektur mit KI-gestütztem Asset-Remastering (RX 7900 XTX Vulkan) und Zero-Config-Online-Multiplayer.
- **Completed Scope (Meilensteine 1–12):**
  1. **`rts-sdk`:** Mod-Manifest, TOML-Schemata, Cross-Reference-Validator & duale Rhai/Lua-Skript-Engine.
  2. **`rts-asset-pipeline`:** Multi-Node-`.glb`-Generator sowie Konverter für Westwood (`.mix`, `.vxl` Voxel-Isosurface, `.shp` 3D-Relief/Turntable, `.ini`, `.wav`) und OpenRA (`MiniYAML`).
  3. **`rts-simulation`:** Deterministischer 20-Hz-Simulationskern (`SimWorld`, 64-Bit FNV-1a `state_hash()`) mit Land-/Wasser-/Luft-Pathfinding, kombiniertem Kampf, Superwaffen, Garnisonen, Spionage & Skirmish-KI.
  4. **`rts-network` & `centralwebserver/`:** TCP-Lockstep-Multiplayer, `.replay.json`-System, 3-Stufen-NAT/UPnP-Traversal (`nat_upnp.rs`) & Reverse-TCP-Relay-Tunnel (`27900–27999`) auf `rust-rts.bls-isp.net`.
  5. **`rts-client`:** Bevy 0.15 3D-Client mit PBR-Terrain, `.glb`-Szenen-Sync, 3D-VFX, Audio, RA2-HUD mit KI-upscaled Cameos, Radar-Minimap & In-Game Engine Studio.
  6. **`rust-ra2/`:** Vollständige Total Conversion (Allies, Soviet, Yuri; 48 Einheiten, 42 Gebäude, 46 Waffen, 54 `.toml`-Karten + 298 `.map`-Dateien, 144 `.glb`-3D-Modelle, 404 Audio-Tracks, 138 4K/HD-30fps-Videos & 827 KI-remasterte Texturen).
  7. **`ns8-rust-rts/`:** Produktionsreifes Rootless NethServer 8 (NS8) Container-Modul für `rust-rts.bls-isp.net`.
