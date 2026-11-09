# Refactoring-Plan (v2 — Reviewed): Engine/Mod-Entkopplung & Konsolidierung der Kernsysteme (Phase 7)

## 1. Übersicht & Zielsetzung (Confirmed Intent)
- **Outcome:** Radikale Verkleinerung und Entkopplung von `rts-simulation`, `rts-sdk` und `rts-client` durch Zusammenfassung aller verstreuten Einzel-Mechaniken in **5 generische Engine-Kernsysteme** und vollständige Auslagerung aller *Red Alert 2 / Yuri's Revenge*-Spezialregeln in [`rust-ra2/`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml).
- **Clean-Cut-Prinzip:** Keine dauerhaften Legacy-Shims oder doppelten Kompatibilitätsfelder. Alte Einzel-Timer auf [`SimEntity`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/world/mod.rs#L277-L422) und redundante Einzel-Booleans auf [`WeaponDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/combat_defs.rs#L16-L104), [`UnitDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/entities.rs#L229-L369) und [`BuildingDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/entities.rs#L10-L115) werden komplett entfernt; `rust-ra2/*.toml`, `rts-asset-pipeline` und die Tests werden direkt migriert.

---

## 2. Die 5 generischen Engine-Kernsysteme (`rts-sdk`, `rts-simulation`, `rts-client`)

### System 1: Zentrale `CapabilityQueries` & `EntityLifecycle` (`world/queries.rs`, `world/lifecycle.rs`)
- **CapabilityQueries:** Ersetzt ~40 verstreute `if`-Ketten durch 5 zentrale Methoden:
  - `can_entity_act(ent, loaded)`
  - `can_entity_move(ent, loaded)`
  - `can_entity_fire(ent, loaded)`
  - `is_entity_targetable(ent)` / `is_entity_phased_out(ent)`
  - `is_entity_invulnerable(ent)`
- **EntityLifecycle:** Ersetzt den an ~12 Stellen duplizierten Cleanup-Code (Tod, Verkauf, Chrono-Erase, Grinder, Ingenieur/Mind-Control/Trigger-`ChangeOwner`, Boarding, MCV-Transformation) durch zentrale Operationen:
  - `cleanup_entity_links(id)` & `remove_entity_cleanly(id, reason, loaded)`
  - `transfer_entity_owner(id, new_owner, loaded)`
  - `enter_container(unit_id, container_id, loaded)`
  - `transform_entity(id, target_id, loaded)`

### System 2: Universelles `StatusSystem` inkl. Client-VFX/HUD-Metadaten (`StatusEffectDef` & `ActiveBuff`)
- **Engine:** Alle zeitbasierten Zustände (EMP, Chrono-Freeze, Temporal-Phase-Lock, Iron Curtain/Force Shield, Confusion, Tesla-Charge, Grapple, Parasite-Infektion, C4/Ivan-Zeitbombe) laufen über `ent.active_buffs` mit generischen Flags:
  - `stunned`, `immobilized`, `disarmed`, `phased_out`, `invulnerable`, `confused`, `power_charge_boost`
  - `dot_dps`, `delayed_burst_damage`, `delayed_burst_splash`, `on_kill_spawn_unit`
  - `source_entity_id`, `attacker_player_id`
- **Client-Entkopplung (`rts-client`):** Jeder Status trägt optionale Darstellungs-Metadaten (`display_label: Option<String>`, `vfx_ring_color: Option<[f32; 3]>`, `vfx_style: StatusVfxStyle`). Damit entfallen in [`ui/inspector.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/ui/inspector.rs) und [`rendering/gizmos.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/rendering/gizmos.rs) sämtliche hartcodierten RA2-Texte (`"IRON CURTAIN"`, `"GRAPPLED BY SQUID!"`) und Einzel-Timer-Abfragen.

### System 3: Gemeinsame Trait-Substructs für `UnitDef` & `BuildingDef` (`SharedEntityTraits`)
- Beseitigt die doppelte Feldpflege zwischen Einheiten und Gebäuden sowie ~30 doppelte `match &ent.kind { EntityKind::Unit .., EntityKind::Building .. }`-Blöcke in `rts-simulation`:
  - `container: Option<ContainerSpec>` (Kapazität, akzeptierte Rollen, Fireports, Tunnel-Netzwerk, Strom pro Insasse, `passenger_weapon_map`, `passenger_role_weapons`, `engineer_repair_aura_per_sec`)
  - `stealth: Option<StealthSensorSpec>` (Permanent-Cloak, Cloak-/Disguise-Detektor, Cloak-Field-Radius, Psychic-Sensor-Radius, Terrain-/Enemy-Disguise, Reveal-on-Fire)
  - `death_effect: Option<DeathPayloadSpec>` (Explosions-/Meltdown-Schaden & Radius, Strahlung/Giftwolke beim Tod, Survivor-Unit & Chance, Salvage-Drop)
  - `point_defense: Option<PointDefenseSpec>` (Abfang-Radius & Cooldown für Raketen)
  - `support_aura: Option<SupportAuraSpec>` (Heilung/Reparatur pro Rolle, Cash-Trickle, Erz-Bonus, Fahrzeug-Rabatt)

### System 4: Kompositorisches `EffectPayload`-System für Waffen, Fähigkeiten, Superwaffen & Infiltration
- Ersetzt ~25 Spezial-Booleans in [`WeaponDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/combat_defs.rs#L45-L103) sowie feste Superwaffen- und Infiltrations-Sonderpfade durch kompositorische Payloads:
  - `Damage`, `ApplyStatus`, `SpawnHazardZone`, `SpawnUnits`, `TransformUnits`, `Displace`, `ChangeOwner`, `ModifyEconomy`, `InvokeScript`

### System 5: Datengetriebene globale Mod-Regeln (`ModRules` in `mod.toml`)
- Lagert bisher im Rust-Code fest verdrahtete Spielkonstanten in eine `[rules]`-Sektion in `mod.toml` (mit sinnvollen Engine-Defaults) aus:
  - Veterancy-XP-Schwellen (`veteran_xp`, `elite_xp`) & Multiplikatoren
  - Commander-Rang-Schwellen
  - Baukosten-Rückerstattung beim Verkauf (`sell_refund_ratio = 0.5`), Gebäude-Reparaturrate & -kosten
  - Low-Power-Geschwindigkeitsfaktor (`low_power_speed_factor = 0.5`)
  - Zufallskisten-Spawn-Intervall (`crate_spawn_interval_ticks = 600`), Max-Anzahl und gewichteter Crate-Payload-Pool

---

## 3. Abhängigkeitsgraph (Dependency Graph)

```text
Phase 7.1: CapabilityQueries & EntityLifecycle (Tasks 114–115)
    │
    ▼
Phase 7.2: Universelles StatusSystem + Client-VFX/HUD-Entkopplung (Tasks 116–117)
    │
    ▼
Phase 7.3: ModRules (mod.toml), EffectPayload & Auslagerung aller RA2-Regeln nach rust-ra2/ (Tasks 118–119)
    │
    ▼
Phase 7.4: Gemeinsame Trait-Substructs (UnitDef/BuildingDef) & WeaponDef-Clean-Cut (Tasks 120–121)
```

---

## 4. Aufgaben-Übersicht (Tasks 114–121 in [`tasks/todo.md`](file:///d:/projekt/rust-rts/tasks/todo.md))

### Phase 7.1: Zentrale `CapabilityQueries` & `EntityLifecycle`
- [x] **Task 114:** Zentrale Capability-Abfragen (`can_entity_act`, `can_entity_move`, `can_entity_fire`, `is_entity_targetable`, `is_entity_invulnerable`) in [`world/queries.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/world/queries.rs) einführen und alle Subsysteme (`movement`, `combat`, `production`, `commands_*`, `triggers_and_fow`, `ai`) darauf umstellen (inkl. Behebung der offenen Milestone-54-Punkte).
- [x] **Task 115:** Zentrales `EntityLifecycle`-Modul (`world/lifecycle.rs`) für Despawn/Link-Cleanup (`remove_entity_cleanly`), Besitzerwechsel (`transfer_entity_owner`), Container-Boarding (`enter_container`) und Entity-Transformation (`transform_entity`) implementieren.
- **Checkpoint 7.1:** Alle Workspace-Tests & Clippy grün; keine verstreuten Timer- oder Cleanup-Ketten mehr.

### Phase 7.2: Universelles `StatusSystem` & Client-Entkopplung (Clean-Cut auf `SimEntity`)
- [x] **Task 116:** [`StatusEffectDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/common.rs#L56-L71) & [`ActiveBuff`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/world/mod.rs#L250-L274) um generische Flags (Capabilities, DoT, Delayed-Burst, Spawn-on-Kill, Source-Tether) sowie Client-Darstellungsfelder (`display_label`, `vfx_ring_color`, `vfx_style`) erweitern und zentrale Status-Tick-Pipeline bauen.
- [x] **Task 117:** Clean-Cut-Ablösung der ~15 Einzel-Timer auf [`SimEntity`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/world/mod.rs#L341-L421) durch `active_buffs` + Umstellung von [`rts-client/src/ui/inspector.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/ui/inspector.rs) und [`rts-client/src/rendering/gizmos.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-client/src/rendering/gizmos.rs) auf datengetriebene Status-Labels und Status-Ringe.
- **Checkpoint 7.2:** [`SimEntity`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/world/mod.rs#L277-L422) ist um ~15 Spezialfelder kleiner; `rts-client` rendert Status-VFX und HUD-Labels generisch.

### Phase 7.3: `ModRules`, `EffectPayload` & Auslagerung aller RA2-Regeln nach `rust-ra2/`
- [x] **Task 118:** Globale `[rules]`-Sektion (`ModRules`) in `ModManifest` (`mod.toml`) für Veterancy-/Commander-Stufen, Sell-Refund, Reparaturrate, Low-Power-Faktor & Crate-Pool sowie datengetriebene IFV-Passagier-Waffenadaption (`passenger_weapon_map`) einführen; alle hartcodierten Waffen-IDs (`"nuclear_demolition"`, `"dog_bite"`, `"m60_mg"`) aus [`combat/mod.rs`](file:///d:/projekt/rust-rts/engine/crates/rts-simulation/src/combat/mod.rs#L179-L224) entfernen.
- [x] **Task 119:** Kompositorisches `EffectPayload`-System für Support-Powers, Superwaffen & Infiltration implementieren und RA2-spezifische Superwaffen-Logik (inkl. `"conscript"`-Fallback beim Genetic Mutator) nach [`rust-ra2/`](file:///d:/projekt/rust-rts/rust-ra2/mod.toml) auslagern.
- **Checkpoint 7.3:** Keine hartcodierten RA2-Einheiten-/Waffen-IDs oder globalen Magic-Numbers mehr in `rts-simulation`.

### Phase 7.4: Gemeinsame Trait-Substructs (`UnitDef`/`BuildingDef`) & `WeaponDef`-Clean-Cut
- [x] **Task 120:** Clean-Cut-Konsolidierung der On-Hit-Spezialfelder auf [`WeaponDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/combat_defs.rs#L16-L104) über `on_hit_status` / `on_hit_zone` und direkte Migration von `rust-ra2/weapons/weapons.toml`, `rts-asset-pipeline` und Tests.
- [x] **Task 121:** Zusammenführung der doppelten Felder auf [`UnitDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/entities.rs#L229-L369) und [`BuildingDef`](file:///d:/projekt/rust-rts/engine/crates/rts-sdk/src/schema/entities.rs#L10-L115) in gemeinsame Trait-Substructs (`ContainerSpec`, `StealthSensorSpec`, `DeathPayloadSpec`, `PointDefenseSpec`, `SupportAuraSpec`), Bereinigung der `match EntityKind`-Duplikate in `rts-simulation` & `rts-client`, Update von `rust-ra2/units/units.toml` & `buildings.toml` sowie End-to-End-Abnahme.
- **Checkpoint 7.4:** Alle 5 Workspace-Crates kompilieren, testen und validieren mit `0` Fehlern und `0` Warnungen bei deutlich reduzierter Code-Größe.
