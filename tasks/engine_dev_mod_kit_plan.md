# Architektur- & Umsetzungsplan v2 (Phase 8): Komplettierung der RTS-Engine & des Dev/Mod-Kits

## Verbesserungen in Version 2 (nach `/grill-me` Architektur-Review)
1. **Strikter Clean-Cut in `rts-sdk` & `rts-client` (Task 122):** Neben den ~400 Zeilen RA2-Tabellen in `rts-client` werden auch die **~25 verbliebenen flachen Altlast-Felder** auf [`UnitDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/entities.rs#L457-L615) und [`BuildingDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/entities.rs#L92-L207) (die jetzt durch `container`, `stealth`, `death_effect`, `point_defense`, `support_aura` abgedeckt sind) sowie alle RA2-Einheiten-IDs in [`ModRules::default()`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/manifest.rs#L81-L113) restlos gelöscht.
2. **Feldgenaues TOML-Deep-Merge pro `id` & `asset_search_dirs`-Kaskade bei `extends` (Task 124):** Sub-Mods müssen bei `extends = "rust-ra2"` nur die geänderten Felder einer Einheit/Waffe angeben (z. B. `{ id = "grizzly_tank", cost = 650 }`); Katalog-Pfade im Child-Manifest sind optional und `LoadedMod.asset_search_dirs` löst 3D-Modelle, Cameos und Sounds kaskadierend über `[sub_mod_dir, base_mod_dir, ...]` auf.
3. **Automatische Cost-Efficiency-Ausreißer-Warnungen im Balance-Prüfstand (Task 127):** `rts-mod-tools benchmark-balance` markiert automatisch Balance-Ausreißer (z. B. >2,0× Kosteneffizienz gegen Konterklassen).
4. **Immer-aktives Group-Path-Caching & Boid-Steering + schaltbare Formations-Tempokopplung (Task 129):** Path-Caching und lokales Separation-Steering sind immer aktiv; die Formations-Geschwindigkeitskopplung (`formation_locked`) ist per HUD-Toggle / `Ctrl + Rechtsklick` schaltbar, damit schnelles Gefechts-Micro nicht ausgebremst wird.
5. **Volle 1:1-Parität zwischen `.rhai` und `.lua` für Welt-Events & Trigger-Variablen (Task 130):** Beide Skriptsprachen werten `WorldEventContext`, `trigger_vars` und Multi-Action-Listen (`Vec<TriggerAction>`) identisch aus.

---

## Phasen- & Task-Übersicht (Tasks 122–131)

Ausführliche Task-Spezifikationen mit Akzeptanzkriterien und Verifikationsschritten befinden sich in [`tasks/todo.md`](file:///d:/projekt/rust-rts/tasks/todo.md) (**Tasks 122–131**).

- [x] **Task 122:** `rts-sdk`, `rts-client`, `rts-asset-pipeline` & `rust-ra2` — Strikter Clean-Cut auf `UnitDef`/`BuildingDef` & `ModRules::default()` sowie datengetriebene `SubFactionDef`, `VisualDef.icon` & `MissionDef`-Medien statt Client-Hardcoding
- [x] **Task 123:** `rts-sdk`, `rts-client` & `rust-ra2` — Datengetriebenes Audio-, Unit-Voice- & Lokalisierungs-System (`AudioCatalog` & `LocaleCatalog`)
- [x] **Checkpoint 8.1:** `rts-sdk` und `rts-client` sind zu 100 % frei von RA2-Strings und flachen Altlast-Feldern
- [x] **Task 124:** `rts-sdk`, `rts-network` & `rts-asset-pipeline` — Feldgenaue Mod-Vererbung (`extends` mit TOML-Deep-Merge pro `id` & `asset_search_dirs`), deterministischer `content_hash()` & `.rtsmod`-Packer (`pack`/`unpack`)
- [x] **Task 125:** `rts-sdk`, `rts-simulation` & `rts-client` — Transaktionaler Live Hot-Reload (`[F12]`) für TOML-Kataloge, `[rules]` und `.rhai`/`.lua`-Skripte im laufenden Gefecht
- [x] **Checkpoint 8.2:** Sub-Mod-Vererbung, Lockstep-Content-Hash, `.rtsmod`-Packer und Live-Hot-Reload bestehen alle Tests
- [x] **Task 126:** `rts-sdk`, `rts-simulation` & `rts-client` — 3D-Karteneditor v2: Pinselgrößen (`1x1`/`3x3`/`5x5`), Symmetrie-Painting, Katalog-Spawn-/Trigger-Platzierung & `validate_map_playability`
- [x] **Task 127:** `rts-simulation`, `rts-asset-pipeline` & `rts-client` — Deterministischer Headless Balance-Analyzer & Matchup-Simulator (`rts-mod-tools benchmark-balance`) inkl. Cost-Efficiency-Ausreißer-Warnungen
- [x] **Checkpoint 8.3:** Symmetrie-Editor, Playability-Validator und Balance-Prüfstand laufen End-to-End
- [x] **Task 128:** `rts-sdk`, `rts-simulation` & `rust-ra2` — Datengetriebene KI-Profile (`ai/ai_profiles.toml`), Build-Order-Listen, Squad-Templates & `on_ai_tick`-Hooks
- [x] **Task 129:** `rts-simulation` & `rts-client` — Immer-aktives Group-Path-Caching & Boid-Separation-Steering + schaltbare Formations-Geschwindigkeitskopplung (`formation_locked`)
- [x] **Checkpoint 8.4:** KI-Profile und deterministisches Gruppen-Pathfinding/Steering bestehen alle Tests
- [x] **Task 130:** `rts-sdk`, `rts-simulation` & `rts-client` — Kampagnen-Direktor: Stateful `trigger_vars`, `All`/`Any`/`Not`, Kamera-/EVA-Direktor & volle 1:1 Rhai/Lua-Parität für `WorldEventContext`
- [x] **Task 131:** `rts-sdk` & `rts-asset-pipeline` — Automatischer Schema-/Markdown-Referenz-Generator (`rts-mod-tools generate-docs`) & Modder-Handbuch (`docs/MOD_DEV_KIT.md`)
- [x] **Checkpoint 8.5 (Phase 8 Complete):** Alle Tasks 122–131 abgeschlossen, `docs/MOD_DEV_KIT.md` generiert, alle Workspace-Tests & Clippy grün (`0` Fehler, `0` Warnungen)
