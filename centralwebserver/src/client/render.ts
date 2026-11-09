import { COUNTRY_I18N, ENGINE_CRATES_SPEC, Locale, t } from './i18n.js';
import {
  BuildingRecord,
  CommanderRankEntry,
  CountryDescriptor,
  EngineCrateInfo,
  GameDatabase,
  LobbyEntry,
  LobbyStatus,
  MapRadarRecord,
  MatchRecord,
  ModSummary,
  RegistryStats,
  UnitRecord,
} from './types.js';

const CRATES_EN: Record<string, { role: string; highlights: string[] }> = {
  'rts-sdk': {
    role: 'Mod Manifest, TOML Schemas, Cross-Validator & Dual Script Engine (.rhai + .lua)',
    highlights: [
      'Strict separation of engine code and game content (zero hardcoding)',
      'Dual script hooks (on_spawn, on_ability, on_tick) in Rhai 1.21 & Lua',
      'Static tech-tree, weapon & campaign cross-reference validator',
    ],
  },
  'rts-simulation': {
    role: 'Deterministic 20 Hz Fixed-Timestep Simulation Core (Headless-capable)',
    highlights: [
      'Command-driven architecture (MoveUnits, AttackTarget, PlaceBuilding, CastSupportPower)',
      'Collision-free A* pathfinding, bridges/water, garrisons, cloning vats & superweapons',
      '64-bit FNV-1a SimWorld::state_hash() for instant lockstep desync detection',
    ],
  },
  'rts-network': {
    role: 'Lockstep Multiplayer Protocol, Replay Verifier & Central Lobby Registry SDK',
    highlights: [
      'Automatic mod-compatibility handshake (ClientHello / ServerWelcome)',
      'Authoritative ServerTickBundle + WorldStateSync snapshot resync',
      'Deterministic .replay.json recording & bit-identical hash verification',
    ],
  },
  'rts-asset-pipeline': {
    role: 'CLI Toolchain for Modders, Legacy Converter & Procedural 3D .glb Generator',
    highlights: [
      'Binary glTF 2.0 (.glb) generator with articulated turret and rotor sub-nodes',
      'Converter for classic Westwood (.mix/.vxl/.shp/.ini) & OpenRA (MiniYAML) assets',
      'CLI inspector for DPS matrices, tech-trees, and unit scaffolding',
    ],
  },
  'rts-client': {
    role: 'Bevy 0.15 3D PBR Client, Tactical Sidebar HUD & In-Game Engine Studio',
    highlights: [
      '360° yaw orbit camera, 3D raycasting, formation preview & force-shield domes',
      'Direct integration with this central multiplayer registry (--lobby-server)',
      'Built-in replay scrubber with fog-of-war POV switcher & in-game tech encyclopedia [F9]',
    ],
  },
};

const MOD_DESC_DE: Record<string, string> = {
  'rust-ra2':
    'Referenz-Total-Conversion-Mod auf Basis von rts-sdk mit 3 asymmetrischen Fraktionen, 10 Sub-Fraktionen, 70 Einheiten, 76 Gebäuden und 54 Karten.',
  openra_classic:
    'Aus OpenRA-MiniYAML-Definitionen konvertiertes Regelwerk inklusive Waffen-Tabellen und klassischer Skirmish-Karte.',
  openra2:
    'Erweiterte OpenRA-2-Konvertierung mit 3 Fraktionen, 48 Einheiten, 42 Gebäuden und automatisch generierten Rhai-Trait-Skripten.',
  westwood_classic:
    'Direkt aus klassischen INI-, VXL- und SHP-Archiven mittels rts-asset-pipeline konvertiertes Referenzpaket.',
};

export function escapeHtml(raw: string | number | undefined | null): string {
  return String(raw ?? '').replace(
    /[&<>"']/g,
    (ch) =>
      ({
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;',
      })[ch] ?? ch
  );
}

export function formatRelativeTime(
  unixMs: number,
  nowMs = Date.now(),
  lang: Locale = 'de'
): string {
  const diffSec = Math.max(0, Math.floor((nowMs - unixMs) / 1000));
  if (lang === 'en') {
    if (diffSec < 5) return 'just now';
    if (diffSec < 60) return `${diffSec}s ago`;
    const diffMin = Math.floor(diffSec / 60);
    if (diffMin < 60) return `${diffMin}m ago`;
    const diffHours = Math.floor(diffMin / 60);
    return `${diffHours}h ${diffMin % 60}m ago`;
  }
  if (diffSec < 5) return 'gerade eben';
  if (diffSec < 60) return `vor ${diffSec}s`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `vor ${diffMin}m`;
  const diffHours = Math.floor(diffMin / 60);
  return `vor ${diffHours}h ${diffMin % 60}m`;
}

export function formatDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}m ${secs.toString().padStart(2, '0')}s`;
}

export function formatStatusLabel(status: LobbyStatus): string {
  switch (status) {
    case 'open':
      return 'OPEN';
    case 'full':
      return 'FULL';
    case 'in_game':
      return 'IN GAME';
  }
}

export function factionCssClass(faction: string): string {
  const lower = (faction || '').toLowerCase();
  if (lower.includes('soviet') || lower.includes('sowjet')) {
    return 'faction-soviet';
  }
  if (lower.includes('yuri') || lower.includes('psi')) {
    return 'faction-yuri';
  }
  if (lower.includes('shared') || lower.includes('neutral')) {
    return 'faction-neutral';
  }
  return 'faction-allied';
}

export function factionDisplayName(
  faction: string,
  lang: Locale = 'de'
): string {
  const lower = (faction || '').toLowerCase();
  if (lower.includes('soviet') || lower.includes('sowjet')) {
    return lang === 'de' ? 'Sowjetunion' : 'Soviet Union';
  }
  if (lower.includes('yuri') || lower.includes('psi')) {
    return lang === 'de' ? 'Yuri-Fraktion' : 'Yuri Faction';
  }
  if (lower.includes('shared') || lower.includes('neutral')) {
    return lang === 'de' ? 'Neutral / Tech' : 'Neutral / Tech';
  }
  return lang === 'de' ? 'Alliierte' : 'Allied Nations';
}

export function renderHeroCrateDetailHtml(
  crateId: string,
  lang: Locale = 'de'
): string {
  const spec =
    ENGINE_CRATES_SPEC.find((c) => c.id === crateId) || ENGINE_CRATES_SPEC[0];
  if (!spec) return '';

  const typesHtml = spec.keyTypes
    .map((ty: string) => `<code class="crate-type-pill">${escapeHtml(ty)}</code>`)
    .join('');

  const guaranteesHtml = spec.guarantees[lang]
    .map((g: string) => `<li>${escapeHtml(g)}</li>`)
    .join('');

  return `
    <div class="crate-detail-header">
      <div>
        <span class="crate-layer-badge">${escapeHtml(spec.layer[lang])}</span>
        <h3 class="crate-detail-title"><code>${escapeHtml(spec.name)}</code></h3>
        <div class="crate-detail-role"><code>${escapeHtml(spec.path)}</code> · ${escapeHtml(spec.loc)}</div>
      </div>
    </div>
    <p class="crate-detail-summary">${escapeHtml(spec.summary[lang])}</p>
    <div class="crate-detail-block">
      <div class="crate-block-label">${lang === 'de' ? 'Zentrale Rust-Typen & Schnittstellen' : 'Core Rust Types & Traits'}</div>
      <div class="crate-type-pills">${typesHtml}</div>
    </div>
    <div class="crate-detail-block">
      <div class="crate-block-label">${lang === 'de' ? 'Architektur- & Determinismus-Garantien' : 'Architecture & Determinism Guarantees'}</div>
      <ul class="crate-guarantees-list">${guaranteesHtml}</ul>
    </div>
    <div class="crate-detail-footer">
      <code class="cli-inline" title="${escapeHtml(spec.cliExample)}">${escapeHtml(spec.cliExample)}</code>
      <button
        type="button"
        class="btn btn-xs btn-secondary"
        data-copy="${escapeHtml(spec.cliExample)}"
      >
        ${escapeHtml(t(lang, 'btn_copy'))}
      </button>
    </div>
  `;
}

export function renderPipelineSpecTabHtml(
  tab: 'schema' | 'scripting' | 'assets' | 'network',
  lang: Locale = 'de'
): string {
  if (tab === 'schema') {
    const title =
      lang === 'de'
        ? 'Deklaratives TOML-Datenmodell & Statische Kreuzvalidierung (rts-sdk)'
        : 'Declarative TOML Data Schema & Static Cross-Validation (rts-sdk)';
    const desc =
      lang === 'de'
        ? 'Alle Entitäten, Fraktionen, Waffen, Schadensmultiplikatoren, Superwaffen und Kampagnen-Trigger werden aus typsicheren TOML-Dateien geladen. Beim Start prüft ModPackage::validate() sämtliche Referenzen zwischen Tech-Tree, Waffen, Projektilen und Skripten vor dem ersten Simulations-Tick.'
        : 'All entities, factions, weapons, armor damage multipliers, support powers, and campaign triggers are loaded from strongly typed TOML manifests. At startup, ModPackage::validate() verifies all cross-references across tech-trees, weapons, projectiles, and scripts before tick 0.';
    const bullets =
      lang === 'de'
        ? [
            'Vollständige Entkopplung: Die Engine kompiliert ohne ein einziges hardcodiertes Fraktions- oder Einheiten-Symbol.',
            'Deterministische Rüstungs- & Schadensmatrix (none, light, medium, heavy, structure) mit expliziten Splash-Radien.',
            'Mehrstufige Produktionsvoraussetzungen (prerequisites), Tech-Stufen, Strombilanzen und Garnisonskapazitäten.',
            'CLI-Prüfung via rts-asset-pipeline validate & balance für automatisierte CI-Qualitätssicherung von Mods.',
          ]
        : [
            'Total decoupling: The engine compiles without a single hardcoded faction, unit, or weapon identifier.',
            'Deterministic armor & damage matrix (none, light, medium, heavy, structure) with explicit splash falloff radii.',
            'Multi-stage production prerequisites, tech levels, power grid deltas, and garrison capacities.',
            'CLI verification via rts-asset-pipeline validate & balance for automated CI quality gates on mod repositories.',
          ];

    const codeSample = `# mod.toml — Declarative Mod Package Manifest
[mod]
id = "rust-ra2"
name = "Red Alert 2 & Yuri's Revenge"
version = "1.0.0"
default_map = "mp_01_offense_defense"

# data/units.toml — Unit Archetype & Combat Definition
[[units]]
id = "grizzly_tank"
name = "Grizzly Battle Tank"
faction = "allies"
role = "tank"
cost = 700
build_time_secs = 10.0
health = 400.0
armor = "heavy"
speed = 7.5
vision_range = 8.0
primary_weapon = "cannon_105mm"
prerequisites = ["allied_war_factory"]

[units.visual]
archetype = "tank"
model = "assets/models/grizzly_tank.glb"
primary_color = [0.22, 0.48, 0.88]`;

    return `
      <div class="pipeline-grid">
        <div class="pipeline-info-col">
          <span class="section-kicker">01 · RTS-SDK DATA SCHEMA</span>
          <h3>${escapeHtml(title)}</h3>
          <p class="pipeline-lead">${escapeHtml(desc)}</p>
          <ul class="pipeline-feature-list">
            ${bullets.map((b) => `<li>${escapeHtml(b)}</li>`).join('')}
          </ul>
          <div class="pipeline-cli-box">
            <span class="cli-box-label">${lang === 'de' ? 'Mod-Paket statisch validieren:' : 'Validate mod package statically:'}</span>
            <div class="cli-box-row">
              <code>cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- validate --mod-path rust-ra2</code>
              <button type="button" class="btn btn-xs btn-secondary" data-copy="cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- validate --mod-path rust-ra2">${escapeHtml(t(lang, 'btn_copy'))}</button>
            </div>
          </div>
        </div>
        <div class="code-panel">
          <div class="code-panel-header">
            <span>TOML SCHEMA · mod.toml / data/units.toml</span>
            <span class="tag tag-command">serde + toml</span>
          </div>
          <pre class="code-block"><code>${escapeHtml(codeSample)}</code></pre>
        </div>
      </div>
    `;
  }

  if (tab === 'scripting') {
    const title =
      lang === 'de'
        ? 'Dual-Sandbox-Scripting: Rhai 1.21 & Lua ohne Desync-Risiko'
        : 'Dual Sandbox Scripting: Rhai 1.21 & Lua Without Desync Risk';
    const desc =
      lang === 'de'
        ? 'Mod-Logik wird in isolierten Rhai- (.rhai) oder Lua- (.lua) Sandboxes ausgeführt. Skripte mutieren den Simulationszustand niemals direkt, sondern geben deterministische ScriptAction-Vektoren zurück, die vom Simulationskern in kanonischer Reihenfolge angewendet werden.'
        : 'Custom mod logic executes inside isolated Rhai (.rhai) or Lua (.lua) sandboxes. Scripts never mutate world state directly; instead, they return deterministic ScriptAction vectors that the simulation kernel applies in canonical entity order.';
    const bullets =
      lang === 'de'
        ? [
            'Drei deterministische Lifecycle-Hooks pro Entität: on_spawn(ctx), on_ability(ctx) und on_tick(ctx).',
            'Kein Zugriff auf Systemzeit, Netzwerk-Sockets, Dateisystem oder nicht-deterministischen Zufall (RNG).',
            'Atomare Rückgabe-Aktionen:HealSelf, ApplyBuff, DamageArea, SpawnUnits, SpawnProjectiles und GrantResources.',
            'Automatische Erkennung der Skript-Engine anhand der Dateiendung (.rhai oder .lua) im Verzeichnis scripts/.',
          ]
        : [
            'Three deterministic lifecycle hooks per entity: on_spawn(ctx), on_ability(ctx), and on_tick(ctx).',
            'Zero access to wall-clock time, sockets, filesystem, or non-deterministic random number generators.',
            'Atomic return actions: HealSelf, ApplyBuff, DamageArea, SpawnUnits, SpawnProjectiles, and GrantResources.',
            'Automatic script runtime dispatch by file extension (.rhai or .lua) inside the mod scripts/ directory.',
          ];

    const codeSample = `// scripts/chrono_legionnaire.rhai — Deterministic Hook
fn on_ability(ctx) {
    let actions = [];
    if ctx.ability_id == "chrono_shift" {
        // Phase buff + localized temporal suppression
        actions.push(#{
            action: "ApplyBuff",
            speed_multiplier: 1.45,
            armor_multiplier: 1.25,
            duration_secs: 6.0
        });
        actions.push(#{
            action: "DamageArea",
            radius: 3.5,
            damage: 120.0
        });
    }
    actions
}`;

    return `
      <div class="pipeline-grid">
        <div class="pipeline-info-col">
          <span class="section-kicker">02 · DUAL SCRIPT SANDBOX</span>
          <h3>${escapeHtml(title)}</h3>
          <p class="pipeline-lead">${escapeHtml(desc)}</p>
          <ul class="pipeline-feature-list">
            ${bullets.map((b) => `<li>${escapeHtml(b)}</li>`).join('')}
          </ul>
          <div class="pipeline-cli-box">
            <span class="cli-box-label">${lang === 'de' ? 'DPS- & Balance-Bericht inkl. Skripten prüfen:' : 'Inspect DPS & balance report including scripts:'}</span>
            <div class="cli-box-row">
              <code>cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- balance --mod-path rust-ra2</code>
              <button type="button" class="btn btn-xs btn-secondary" data-copy="cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- balance --mod-path rust-ra2">${escapeHtml(t(lang, 'btn_copy'))}</button>
            </div>
          </div>
        </div>
        <div class="code-panel">
          <div class="code-panel-header">
            <span>RHAI 1.21 / LUA · ScriptAction Sandbox</span>
            <span class="tag tag-verified">DETERMINISTIC</span>
          </div>
          <pre class="code-block"><code>${escapeHtml(codeSample)}</code></pre>
        </div>
      </div>
    `;
  }

  if (tab === 'assets') {
    const title =
      lang === 'de'
        ? 'Prozedurale 3D-.glb-Synthese & Legacy-Konverter (rts-asset-pipeline)'
        : 'Procedural 3D .glb Synthesis & Legacy Converter (rts-asset-pipeline)';
    const desc =
      lang === 'de'
        ? 'Das Crate rts-asset-pipeline erzeugt valide binäre glTF-2.0-Modelle (.glb) mit hierarchischen Sub-Nodes für unabhängig rotierende Geschütztürme, Geschützrohre und Helikopter-Rotoren und konvertiert klassische Westwood- (.mix, .vxl, .shp, .ini) sowie OpenRA-Archive.'
        : 'The rts-asset-pipeline crate synthesizes binary glTF 2.0 (.glb) meshes with hierarchical sub-nodes for independently rotating turrets, barrels, and helicopter rotors, and converts legacy Westwood (.mix, .vxl, .shp, .ini) and OpenRA archives.';
    const bullets =
      lang === 'de'
        ? [
            'Hierarchische glTF-2.0-Knoten: Hull -> Turret -> Barrel für präzises Anvisieren in Bevy 0.15 ohne Mesh-Deformation.',
            'PBR-Materialattribute (BaseColorFactor, MetallicRoughness, Fraktions-Teamfarben) direkt im .glb eingebettet.',
            'Automatischer Konverter (import-mod) für bestehende OpenRA-MiniYAML- und klassische INI/VXL/SHP-Bestände.',
            'Headless-kompatibel: Der Simulationskern benötigt keine 3D-Assets und läuft auf Servern mit minimalem Speicherprofil.',
          ]
        : [
            'Hierarchical glTF 2.0 nodes: Hull -> Turret -> Barrel for accurate target tracking in Bevy 0.15 without skinning overhead.',
            'PBR material attributes (BaseColorFactor, MetallicRoughness, faction team-color tinting) embedded directly in .glb.',
            'Automated importer (import-mod) for existing OpenRA MiniYAML and classic INI/VXL/SHP asset repositories.',
            'Headless-compatible: The simulation core requires zero 3D assets to run on dedicated servers.',
          ];

    const codeSample = `# 1. Generate articulated 3D .glb models for all mod entities
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- \\
  generate-3d-assets --mod-path rust-ra2

# 2. Import legacy Westwood (.ini/.vxl/.shp) or OpenRA mod directory
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- \\
  import-mod --source ./legacy-mod --output ./mods/custom-mod

# 3. Scaffold a new unit with TOML definition, Rhai hook & .glb model
cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- \\
  new-unit --mod-path rust-ra2 --id siege_mech --role tank`;

    return `
      <div class="pipeline-grid">
        <div class="pipeline-info-col">
          <span class="section-kicker">03 · 3D GLTF 2.0 ASSET PIPELINE</span>
          <h3>${escapeHtml(title)}</h3>
          <p class="pipeline-lead">${escapeHtml(desc)}</p>
          <ul class="pipeline-feature-list">
            ${bullets.map((b) => `<li>${escapeHtml(b)}</li>`).join('')}
          </ul>
          <div class="pipeline-cli-box">
            <span class="cli-box-label">${lang === 'de' ? '3D-.glb-Assets für Mod generieren:' : 'Generate 3D .glb assets for mod:'}</span>
            <div class="cli-box-row">
              <code>cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- generate-3d-assets --mod-path rust-ra2</code>
              <button type="button" class="btn btn-xs btn-secondary" data-copy="cargo run --manifest-path engine/Cargo.toml -p rts-asset-pipeline -- generate-3d-assets --mod-path rust-ra2">${escapeHtml(t(lang, 'btn_copy'))}</button>
            </div>
          </div>
        </div>
        <div class="code-panel">
          <div class="code-panel-header">
            <span>CLI TOOLCHAIN · rts-asset-pipeline</span>
            <span class="tag tag-command">glTF 2.0 Binary (.glb)</span>
          </div>
          <pre class="code-block"><code>${escapeHtml(codeSample)}</code></pre>
        </div>
      </div>
    `;
  }

  const title =
    lang === 'de'
      ? '20 Hz Lockstep-Netzwerkprotokoll & Reverse-TCP NAT-Relay (rts-network)'
      : '20 Hz Lockstep Network Protocol & Reverse-TCP NAT Relay (rts-network)';
  const desc =
    lang === 'de'
      ? 'Anstelle bandbreitenintensiver Objekt-Replikation überträgt rts-network ausschließlich kompakte SimCommand-Pakete in autoritativen ServerTickBundles (50 ms). Jeder Tick wird mittels 64-Bit FNV-1a-Zustands-Hash gegen Desynchronisation gesichert.'
      : 'Instead of bandwidth-heavy entity replication, rts-network transmits compact SimCommand frames inside authoritative ServerTickBundles (50 ms). Every tick is verified against desynchronization via a 64-bit FNV-1a state hash.';
  const bullets =
    lang === 'de'
      ? [
          'Automatischer Mod-Handshake (ClientHello / ServerWelcome) stellt identische Mod-ID und Version vor Spielstart sicher.',
          '64-Bit FNV-1a SimWorld::state_hash() über Ressourcen, Einheiten, Gebäude, Projektile und Superwaffen-Timer.',
          'Integrierter Reverse-TCP-Tunnel auf rust-rts.bls-isp.net ermöglicht Hosten hinter Carrier-Grade-NAT ohne Port-Forwarding.',
          'Deterministische .replay.json-Aufzeichnung und Re-Simulations-Prüfung zur lückenlosen Verifikation von Turnierspielen.',
        ]
      : [
          'Automatic mod handshake (ClientHello / ServerWelcome) enforces matching mod ID and version prior to match start.',
          '64-bit FNV-1a SimWorld::state_hash() across resources, units, buildings, projectiles, and superweapon timers.',
          'Integrated Reverse-TCP tunnel on rust-rts.bls-isp.net enables hosting behind Carrier-Grade NAT without port forwarding.',
          'Deterministic .replay.json recording and re-simulation verifier for tamper-proof tournament match auditing.',
        ];

  const codeSample = `# Host a public multiplayer lobby registered on rust-rts.bls-isp.net
cargo run --manifest-path engine/Cargo.toml -p rts-client -- \\
  --mod-path rust-ra2 \\
  --host 0.0.0.0:7878 \\
  --lobby-server https://rust-rts.bls-isp.net \\
  --lobby-name "EU Lockstep #1"

# Join an active session or verify a recorded .replay.json
cargo run --manifest-path engine/Cargo.toml -p rts-client -- \\
  --mod-path rust-ra2 \\
  --verify-replay match_01.replay.json`;

  return `
    <div class="pipeline-grid">
      <div class="pipeline-info-col">
        <span class="section-kicker">04 · LOCKSTEP & NAT RELAY</span>
        <h3>${escapeHtml(title)}</h3>
        <p class="pipeline-lead">${escapeHtml(desc)}</p>
        <ul class="pipeline-feature-list">
          ${bullets.map((b) => `<li>${escapeHtml(b)}</li>`).join('')}
        </ul>
        <div class="pipeline-cli-box">
          <span class="cli-box-label">${lang === 'de' ? 'Öffentliche Lobby über Master-Registry hosten:' : 'Host public lobby via master registry:'}</span>
          <div class="cli-box-row">
            <code>cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --host 0.0.0.0:7878 --lobby-server https://rust-rts.bls-isp.net</code>
            <button type="button" class="btn btn-xs btn-secondary" data-copy="cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --host 0.0.0.0:7878 --lobby-server https://rust-rts.bls-isp.net">${escapeHtml(t(lang, 'btn_copy'))}</button>
          </div>
        </div>
      </div>
      <div class="code-panel">
        <div class="code-panel-header">
          <span>LOCKSTEP PROTOCOL · FNV-1a 64-Bit Hash</span>
          <span class="tag tag-verified">20 HZ TICK</span>
        </div>
        <pre class="code-block"><code>${escapeHtml(codeSample)}</code></pre>
      </div>
    </div>
  `;
}

export function renderLobbyRowsHtml(
  lobbies: LobbyEntry[],
  ttlSecs: number,
  nowMs = Date.now(),
  lang: Locale = 'de'
): string {
  if (lobbies.length === 0) {
    return `
      <tr>
        <td colspan="7" class="empty-cell">
          <div class="empty-state" role="status">
            <div class="empty-title">${escapeHtml(t(lang, 'lobbies_empty_title'))}</div>
            <p class="empty-desc">
              ${t(lang, 'lobbies_empty_desc_html')}
            </p>
            <div class="empty-actions">
              <a href="#sdk-pipeline" class="btn btn-primary">
                ${escapeHtml(t(lang, 'lobbies_empty_btn_guide'))}
              </a>
              <a href="#data-inspector" class="btn btn-secondary">
                ${escapeHtml(t(lang, 'lobbies_empty_btn_maps'))}
              </a>
            </div>
          </div>
        </td>
      </tr>
    `;
  }

  return lobbies
    .map((lobby) => {
      const ageSec = Math.max(
        0,
        Math.floor((nowMs - lobby.last_heartbeat_unix_ms) / 1000)
      );
      const remainingTtl = Math.max(0, ttlSecs - ageSec);
      const primaryEndpoint =
        lobby.external_addr || lobby.relay_addr || lobby.host_addr;
      const connectCmd = `cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path ${lobby.mod_id} --connect ${primaryEndpoint}`;
      const lockBadge = lobby.password_protected
        ? `<span class="tag tag-warn" title="${lang === 'de' ? 'Passwortgeschützt' : 'Password Protected'}">AUTH</span>`
        : '';
      const natBadge = `<span class="tag tag-verified" title="NAT Mode">${escapeHtml(lobby.nat_mode || 'Direct P2P')}</span>`;
      const relaySub = lobby.relay_addr
        ? `<div class="cell-sub">BLS-ISP Relay: <code>${escapeHtml(lobby.relay_addr)}</code></div>`
        : '';
      const slotPct = Math.min(
        100,
        Math.round((lobby.current_players / Math.max(1, lobby.max_players)) * 100)
      );

      return `
        <tr data-lobby-id="${escapeHtml(lobby.lobby_id)}">
          <td>
            <span class="status-pill status-${escapeHtml(lobby.status)}">
              <span class="status-dot" aria-hidden="true"></span>
              ${formatStatusLabel(lobby.status)}
            </span>
          </td>
          <td>
            <div class="lobby-title-cell">
              <strong>${escapeHtml(lobby.lobby_name)}</strong>
              ${lockBadge}
            </div>
            <div class="cell-sub">Sync ${formatRelativeTime(lobby.last_heartbeat_unix_ms, nowMs, lang)} · TTL ${remainingTtl}s</div>
          </td>
          <td>
            <span class="host-name">${escapeHtml(lobby.host_player)}</span>
          </td>
          <td>
            <button
              type="button"
              class="map-preview-link"
              data-preview-map="${escapeHtml(lobby.map_id)}"
              title="${escapeHtml(t(lang, 'map_radar_tooltip'))}"
            >
              ${escapeHtml(lobby.map_name)}
            </button>
            <div class="cell-sub"><code>${escapeHtml(lobby.map_id)}</code></div>
          </td>
          <td>
            <span class="tag tag-mod">${escapeHtml(lobby.mod_id)}</span>
            <span class="version-label">v${escapeHtml(lobby.mod_version)}</span>
          </td>
          <td>
            <div class="slot-meter" aria-label="${lobby.current_players} / ${lobby.max_players}">
              <div class="slot-meter-top">
                <span class="slot-count">${lobby.current_players} / ${lobby.max_players}</span>
                <span class="slot-mode">${escapeHtml(lobby.game_mode)}</span>
              </div>
              <div class="slot-bar-track">
                <div class="slot-bar-fill" style="width:${slotPct}%"></div>
              </div>
            </div>
          </td>
          <td>
            <div class="connect-box">
              <code class="connect-code">${escapeHtml(primaryEndpoint)}</code>
              ${natBadge}
              <button
                type="button"
                class="btn btn-xs btn-secondary"
                data-copy="${escapeHtml(primaryEndpoint)}"
              >
                ${escapeHtml(t(lang, 'btn_copy_ip'))}
              </button>
              <button
                type="button"
                class="btn btn-xs btn-ghost"
                data-copy="${escapeHtml(connectCmd)}"
              >
                ${escapeHtml(t(lang, 'btn_direct_join'))}
              </button>
            </div>
            ${relaySub}
          </td>
        </tr>
      `;
    })
    .join('');
}

export function renderLeaderboardHtml(
  leaderboard: CommanderRankEntry[],
  nowMs = Date.now(),
  lang: Locale = 'de'
): string {
  if (leaderboard.length === 0) {
    return `
      <tr>
        <td colspan="7" class="empty-cell">
          ${lang === 'de' ? 'Noch keine abgeschlossenen Spiele verzeichnet.' : 'No completed matches recorded yet.'}
        </td>
      </tr>
    `;
  }

  const localeTag = lang === 'de' ? 'de-DE' : 'en-US';
  const winShort = lang === 'de' ? 'S' : 'W';
  const lossShort = lang === 'de' ? 'N' : 'L';
  const gamesWord = lang === 'de' ? 'Spiele' : 'games';
  const hashVerifiedWord = lang === 'de' ? 'Hash-verifiziert' : 'hash-verified';

  return leaderboard
    .slice(0, 10)
    .map((entry) => {
      const rankBadge =
        entry.rank === 1
          ? '<span class="rank-medal rank-gold">#1</span>'
          : entry.rank === 2
            ? '<span class="rank-medal rank-silver">#2</span>'
            : entry.rank === 3
              ? '<span class="rank-medal rank-bronze">#3</span>'
              : `<span class="rank-num">#${entry.rank}</span>`;

      const fClass = factionCssClass(entry.main_faction);

      return `
        <tr>
          <td>${rankBadge}</td>
          <td>
            <strong>${escapeHtml(entry.player_name)}</strong>
          </td>
          <td>
            <span class="faction-badge ${fClass}">${escapeHtml(factionDisplayName(entry.main_faction, lang))}</span>
          </td>
          <td>
            <strong class="text-win">${entry.wins} ${winShort}</strong> · <span class="text-loss">${entry.losses} ${lossShort}</span>
            <span class="cell-sub">(${entry.matches_played} ${gamesWord})</span>
          </td>
          <td>
            <div class="winrate-cell">
              <span class="winrate-val">${entry.win_rate_pct}%</span>
              <div class="slot-bar-track">
                <div class="slot-bar-fill" style="width:${entry.win_rate_pct}%"></div>
              </div>
            </div>
          </td>
          <td>
            <code>${entry.total_ticks.toLocaleString(localeTag)} Ticks</code>
            <div class="cell-sub">${entry.verified_matches}× ${hashVerifiedWord}</div>
          </td>
          <td>
            <span class="cell-sub">${formatRelativeTime(entry.last_played_unix_ms, nowMs, lang)}</span>
          </td>
        </tr>
      `;
    })
    .join('');
}

export function renderMatchesRowsHtml(
  matches: MatchRecord[],
  nowMs = Date.now(),
  lang: Locale = 'de'
): string {
  if (matches.length === 0) {
    return `
      <tr>
        <td colspan="7" class="empty-cell">
          <div class="empty-state" role="status">
            <div class="empty-title">${escapeHtml(t(lang, 'matches_empty_title'))}</div>
            <p class="empty-desc">${t(lang, 'matches_empty_desc_html')}</p>
          </div>
        </td>
      </tr>
    `;
  }

  const localeTag = lang === 'de' ? 'de-DE' : 'en-US';

  return matches
    .map((m) => {
      const factionClass = factionCssClass(m.winner_faction);

      const syncBadge = m.desync_detected
        ? '<span class="tag tag-danger">DESYNC</span>'
        : '<span class="tag tag-verified">BIT-IDENTICAL</span>';

      const playersList = m.players
        .map(
          (p) =>
            `<span class="player-chip ${p.is_winner ? 'player-winner' : ''}">${escapeHtml(p.name)} (${escapeHtml(p.faction)})</span>`
        )
        .join(' ');

      return `
        <tr>
          <td>
            <div class="lobby-title-cell">
              <strong>${escapeHtml(m.lobby_name)}</strong>
            </div>
            <div class="cell-sub">${playersList}</div>
          </td>
          <td>
            <span class="faction-badge ${factionClass}">${escapeHtml(m.winner_player)} (${escapeHtml(m.winner_faction)})</span>
          </td>
          <td>
            <button
              type="button"
              class="map-preview-link"
              data-preview-map="${escapeHtml(m.map_id)}"
            >
              ${escapeHtml(m.map_name)}
            </button>
            <div class="cell-sub"><span class="tag tag-mod">${escapeHtml(m.mod_id)}</span> v${escapeHtml(m.mod_version)}</div>
          </td>
          <td>
            <div><strong>${formatDuration(m.duration_secs)}</strong></div>
            <div class="cell-sub">${m.final_tick.toLocaleString(localeTag)} Ticks · ${m.total_commands} Cmds</div>
          </td>
          <td>
            <code class="hash-code">${escapeHtml(m.final_state_hash)}</code>
          </td>
          <td>${syncBadge}</td>
          <td>
            <div class="cell-sub">${formatRelativeTime(m.completed_at_unix_ms, nowMs, lang)}</div>
            ${m.replay_file ? `<code class="replay-path">${escapeHtml(m.replay_file)}</code>` : '<span class="cell-sub">BLS-ISP Live Registry</span>'}
          </td>
        </tr>
      `;
    })
    .join('');
}

export function renderCountriesStripHtml(
  countries: CountryDescriptor[],
  selectedCountryId: string,
  lang: Locale = 'de'
): string {
  return countries
    .map((c) => {
      const isSelected = c.id === selectedCountryId;
      const fClass = factionCssClass(c.faction);
      const loc = COUNTRY_I18N[c.id];
      const dispName = loc ? loc.name[lang] : c.name;
      const dispUnit = loc ? loc.special_unit[lang] : c.special_unit;

      return `
        <button
          type="button"
          class="country-tab-btn ${fClass} ${isSelected ? 'country-tab-active' : ''}"
          data-country-id="${escapeHtml(c.id)}"
          aria-pressed="${isSelected}"
        >
          <div class="country-tab-meta">
            <strong>${escapeHtml(dispName)}</strong>
            <span class="country-tab-unit">${escapeHtml(dispUnit)}</span>
          </div>
          <img src="${escapeHtml(c.special_cameo)}" alt="" class="country-tab-cameo" loading="lazy">
        </button>
      `;
    })
    .join('');
}

export function renderCountryDetailHtml(
  country: CountryDescriptor,
  lang: Locale = 'de'
): string {
  const fClass = factionCssClass(country.faction);
  const loc = COUNTRY_I18N[country.id];
  const dispName = loc ? loc.name[lang] : country.name;
  const dispUnit = loc ? loc.special_unit[lang] : country.special_unit;
  const dispDoctrine = loc ? loc.doctrine[lang] : country.doctrine;

  return `
    <div class="country-showcase-card ${fClass}">
      <div class="country-artwork-wrap">
        <img
          src="${escapeHtml(country.artwork)}"
          alt="${escapeHtml(dispName)}"
          class="country-artwork-img"
        >
        <div class="country-artwork-overlay">
          <span class="faction-badge ${fClass}">${escapeHtml(factionDisplayName(country.faction, lang))}</span>
          <h3 class="country-dossier-title">${escapeHtml(dispName)}</h3>
        </div>
      </div>
      <div class="country-dossier-body">
        <div class="country-special-row">
          <div>
            <span class="country-special-label">${escapeHtml(t(lang, 'country_special_kicker'))}</span>
            <h4 class="country-special-title">${escapeHtml(dispUnit)}</h4>
          </div>
          <img src="${escapeHtml(country.special_cameo)}" alt="${escapeHtml(dispUnit)}" class="cameo-thumb-lg">
        </div>
        <p class="country-dossier-desc">${escapeHtml(dispDoctrine)}</p>
        <div class="country-meta-grid">
          <div class="meta-stat-box">
            <span class="meta-stat-label">${escapeHtml(t(lang, 'country_meta_alliance'))}</span>
            <strong>${escapeHtml(factionDisplayName(country.faction, lang))}</strong>
          </div>
          <div class="meta-stat-box">
            <span class="meta-stat-label">${escapeHtml(t(lang, 'country_meta_credits'))}</span>
            <strong>$10,000 Credits</strong>
          </div>
          <div class="meta-stat-box">
            <span class="meta-stat-label">${escapeHtml(t(lang, 'country_meta_assets'))}</span>
            <strong>glTF 2.0 (.glb) + TOML</strong>
          </div>
        </div>
      </div>
    </div>
  `;
}

export function renderArsenalGridHtml(
  items: Array<UnitRecord | BuildingRecord>,
  selectedId: string,
  kind: 'units' | 'buildings',
  lang: Locale = 'de'
): string {
  if (items.length === 0) {
    return `<div class="empty-state"><p>${escapeHtml(t(lang, 'arsenal_empty'))}</p></div>`;
  }

  return items
    .map((item) => {
      const isSelected = item.id === selectedId;
      const fClass = factionCssClass(item.faction);
      const roleLabel =
        kind === 'units'
          ? (item as UnitRecord).movement_domain === 'air' ||
            (item as UnitRecord).movement_domain === 'jumpjet'
            ? 'AIR'
            : (item as UnitRecord).movement_domain === 'naval'
              ? 'NAVAL'
              : item.role.toUpperCase()
          : item.role.toUpperCase();

      return `
        <button
          type="button"
          class="cameo-card ${fClass} ${isSelected ? 'cameo-card-active' : ''}"
          data-arsenal-kind="${kind}"
          data-arsenal-id="${escapeHtml(item.id)}"
          aria-pressed="${isSelected}"
        >
          <div class="cameo-img-frame">
            <img src="${escapeHtml(item.cameo)}" alt="${escapeHtml(item.name)}" class="cameo-img" loading="lazy">
            <span class="cameo-cost-badge">$${item.cost}</span>
          </div>
          <div class="cameo-card-info">
            <strong class="cameo-card-name">${escapeHtml(item.name)}</strong>
            <div class="cameo-card-sub">
              <span class="cameo-tier">${escapeHtml(roleLabel)}</span>
            </div>
          </div>
        </button>
      `;
    })
    .join('');
}

export function renderArsenalInspectorHtml(
  item: UnitRecord | BuildingRecord,
  kind: 'units' | 'buildings',
  db: GameDatabase,
  lang: Locale = 'de'
): string {
  const fClass = factionCssClass(item.faction);
  const isUnit = kind === 'units';
  const unit = isUnit ? (item as UnitRecord) : null;
  const bldg = !isUnit ? (item as BuildingRecord) : null;

  const findEntityNameAndCameo = (
    id: string
  ): { id: string; name: string; cameo: string; kind: 'units' | 'buildings' } => {
    const foundB = db.buildings.find((b) => b.id === id);
    if (foundB) {
      return { id: foundB.id, name: foundB.name, cameo: foundB.cameo, kind: 'buildings' };
    }
    const foundU = db.units.find((u) => u.id === id);
    if (foundU) {
      return { id: foundU.id, name: foundU.name, cameo: foundU.cameo, kind: 'units' };
    }
    return { id, name: id, cameo: '/assets/cameos/techicon.webp', kind: 'buildings' };
  };

  const prereqHtml =
    item.prerequisites.length > 0
      ? item.prerequisites
          .map((pid) => {
            const ref = findEntityNameAndCameo(pid);
            return `
              <button type="button" class="tech-pill" data-arsenal-kind="${ref.kind}" data-arsenal-id="${escapeHtml(ref.id)}">
                <img src="${escapeHtml(ref.cameo)}" alt="" class="tech-pill-img">
                <span>${escapeHtml(ref.name)}</span>
              </button>
            `;
          })
          .join('')
      : `<span class="cell-sub">${escapeHtml(t(lang, 'inspector_prereq_none'))}</span>`;

  const unlocksUnits = db.units.filter((u) => u.prerequisites.includes(item.id));
  const unlocksBldgs = db.buildings.filter((b) =>
    b.prerequisites.includes(item.id)
  );
  const allUnlocks = [
    ...unlocksUnits.map((u) => ({ ...u, kind: 'units' as const })),
    ...unlocksBldgs.map((b) => ({ ...b, kind: 'buildings' as const })),
  ];

  const unlocksHtml =
    allUnlocks.length > 0
      ? allUnlocks
          .slice(0, 8)
          .map(
            (un) => `
              <button type="button" class="tech-pill" data-arsenal-kind="${un.kind}" data-arsenal-id="${escapeHtml(un.id)}">
                <img src="${escapeHtml(un.cameo)}" alt="" class="tech-pill-img">
                <span>${escapeHtml(un.name)}</span>
              </button>
            `
          )
          .join('')
      : '';

  const w = item.weapon_details;
  let weaponHtml = '';
  if (w) {
    const mults = w.armor_multipliers || {};
    const armorOrder: Array<[string, string]> = [
      ['none', t(lang, 'armor_vs_none')],
      ['light', t(lang, 'armor_vs_light')],
      ['medium', t(lang, 'armor_vs_medium')],
      ['heavy', t(lang, 'armor_vs_heavy')],
      ['structure', t(lang, 'armor_vs_structure')],
    ];
    const barsHtml = armorOrder
      .map(([key, label]) => {
        const val = Number(mults[key] ?? 1.0);
        const pct = Math.min(100, Math.round((val / 1.8) * 100));
        return `
          <div class="armor-bar-row">
            <span class="armor-bar-label">${escapeHtml(label)}</span>
            <div class="armor-bar-track">
              <div class="armor-bar-fill" style="width:${pct}%"></div>
            </div>
            <strong class="armor-bar-val">${Math.round(val * 100)}%</strong>
          </div>
        `;
      })
      .join('');

    weaponHtml = `
      <div class="inspector-section">
        <div class="inspector-section-title">${escapeHtml(t(lang, 'inspector_weapon_title'))}</div>
        <div class="weapon-summary-box">
          <div class="weapon-header-row">
            <strong>${escapeHtml(w.name)}</strong>
            <span class="weapon-dps-badge">${w.dps} DPS</span>
          </div>
          <div class="weapon-meta-row">
            <span>${escapeHtml(t(lang, 'inspector_dmg'))}: <strong>${w.damage}${w.burst > 1 ? ` ×${w.burst}` : ''}</strong></span>
            <span>${escapeHtml(t(lang, 'inspector_range'))}: <strong>${w.range}</strong></span>
            <span>${escapeHtml(t(lang, 'inspector_cooldown'))}: <strong>${w.cooldown_secs}s</strong></span>
          </div>
          <div class="armor-bars">
            ${barsHtml}
          </div>
        </div>
      </div>
    `;
  }

  let specialHtml = '';
  if (unit && unit.abilities.length > 0) {
    specialHtml = `
      <div class="inspector-section">
        <div class="inspector-section-title">${escapeHtml(t(lang, 'inspector_abilities_title'))}</div>
        <div class="tech-pills-wrap">
          ${unit.abilities
            .map(
              (ab) => `
              <div class="ability-chip">
                <strong>${escapeHtml(ab.name)}</strong> · ${ab.cooldown_secs}s
              </div>
            `
            )
            .join('')}
        </div>
      </div>
    `;
  } else if (bldg && bldg.support_powers.length > 0) {
    specialHtml = `
      <div class="inspector-section">
        <div class="inspector-section-title">${escapeHtml(t(lang, 'inspector_superweapons_title'))}</div>
        <div class="tech-pills-wrap">
          ${bldg.support_powers
            .map(
              (sp) => `
              <div class="ability-chip">
                <strong>${escapeHtml(sp.name)}</strong> · ${sp.cooldown_secs}s (R=${sp.radius})
              </div>
            `
            )
            .join('')}
        </div>
      </div>
    `;
  }

  return `
    <div class="inspector-card ${fClass}">
      <div class="inspector-header">
        <img src="${escapeHtml(item.cameo)}" alt="${escapeHtml(item.name)}" class="inspector-cameo-lg">
        <div class="inspector-title-block">
          <div class="inspector-badges">
            <span class="faction-badge ${fClass}">${escapeHtml(factionDisplayName(item.faction, lang))}</span>
          </div>
          <h3>${escapeHtml(item.name)}</h3>
          <div class="inspector-id-sub">ID: <code>${escapeHtml(item.id)}</code> · 3D: <code>${escapeHtml(item.visual.model || `${item.id}.glb`)}</code></div>
        </div>
      </div>

      <div class="turntable-viewport-wrap">
        <canvas id="inspector-3d-canvas" width="520" height="270" aria-label="3D Turntable ${escapeHtml(item.name)}"></canvas>
        <div class="turntable-overlay-hint">
          <span>${escapeHtml(t(lang, 'inspector_drag_hint'))}</span>
        </div>
      </div>

      <div class="inspector-kpi-grid">
        <div class="inspector-kpi">
          <span>${escapeHtml(t(lang, 'inspector_cost'))}</span>
          <strong>$${item.cost}</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(t(lang, 'inspector_build_time'))}</span>
          <strong>${item.build_time_secs}s</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(t(lang, 'inspector_hp'))}</span>
          <strong>${item.health} HP</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(t(lang, 'inspector_armor'))}</span>
          <strong>${escapeHtml(item.armor.toUpperCase())}</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(isUnit ? t(lang, 'inspector_speed') : t(lang, 'inspector_power'))}</span>
          <strong>${isUnit ? `${unit!.speed} m/s` : `${bldg!.power_delta >= 0 ? '+' : ''}${bldg!.power_delta} P`}</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(isUnit ? t(lang, 'inspector_vision') : t(lang, 'inspector_footprint'))}</span>
          <strong>${isUnit ? `${unit!.vision_range} Tiles` : `${bldg!.footprint[0]}×${bldg!.footprint[1]} Tiles`}</strong>
        </div>
      </div>

      ${weaponHtml}
      ${specialHtml}

      <div class="inspector-section">
        <div class="inspector-section-title">${escapeHtml(t(lang, 'inspector_prereq_title'))}</div>
        <div class="tech-pills-wrap">${prereqHtml}</div>
      </div>

      ${
        unlocksHtml
          ? `
        <div class="inspector-section">
          <div class="inspector-section-title">${escapeHtml(t(lang, 'inspector_unlocks_title'))} (${allUnlocks.length})</div>
          <div class="tech-pills-wrap">${unlocksHtml}</div>
        </div>
      `
          : ''
      }
    </div>
  `;
}

const mapPreviewImageCache = new Map<string, HTMLImageElement>();
const mapPreviewLoading = new Set<string>();

export function renderTacticalMapCanvas(
  canvas: HTMLCanvasElement,
  map: MapRadarRecord
): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const grad = ctx.createLinearGradient(0, 0, w, h);
  grad.addColorStop(0, '#060c14');
  grad.addColorStop(0.5, '#091420');
  grad.addColorStop(1, '#050a11');
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, w, h);

  const webpUrl = '/' + (map.preview_webp || `assets/maps/${map.id}.webp`).replace(/^\/+/, '');
  const cachedImg = mapPreviewImageCache.get(webpUrl);

  if (cachedImg && cachedImg.complete && cachedImg.naturalWidth > 0) {
    ctx.drawImage(cachedImg, 0, 0, w, h);

    // Top HUD telemetry bar
    ctx.fillStyle = 'rgba(4, 9, 16, 0.84)';
    ctx.fillRect(0, 0, w, 28);
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.35)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, 28.5);
    ctx.lineTo(w, 28.5);
    ctx.stroke();

    ctx.font = 'bold 11px Consolas, monospace';
    ctx.textBaseline = 'middle';
    ctx.textAlign = 'left';
    ctx.fillStyle = '#38bdf8';
    const theater = (map.theater || 'TEMPERATE').toUpperCase();
    const elevMin = map.elevation_min ?? 0;
    const elevMax = map.elevation_max ?? 4;
    ctx.fillText(
      `ISO-MAPPACK5 // THEATER: ${theater} · ELEV: ${elevMin}..${elevMax}`,
      10,
      14
    );

    ctx.textAlign = 'right';
    ctx.fillStyle = '#e2e8f0';
    ctx.fillText(
      `${map.width}×${map.height} CELLS · ${map.max_players}P`,
      w - 10,
      14
    );

    // Bottom HUD telemetry bar
    ctx.fillStyle = 'rgba(4, 9, 16, 0.84)';
    ctx.fillRect(0, h - 26, w, 26);
    ctx.strokeStyle = 'rgba(56, 189, 248, 0.28)';
    ctx.beginPath();
    ctx.moveTo(0, h - 26.5);
    ctx.lineTo(w, h - 26.5);
    ctx.stroke();

    ctx.textAlign = 'left';
    ctx.fillStyle = '#94a3b8';
    ctx.font = '11px Consolas, monospace';
    ctx.fillText(
      `STRUCTURES: ${map.structures_count ?? map.tech_structures.length} · TREES: ${map.trees_count ?? 0} · ORE DEPOSITS: ${map.ore_patches.length}`,
      10,
      h - 13
    );

    ctx.textAlign = 'right';
    ctx.fillStyle = '#facc15';
    ctx.fillText(`ID: ${map.id}`, w - 10, h - 13);

    ctx.strokeStyle = '#2c4468';
    ctx.lineWidth = 2;
    ctx.strokeRect(1, 1, w - 2, h - 2);
    return;
  }

  if (!mapPreviewLoading.has(webpUrl) && typeof Image !== 'undefined') {
    mapPreviewLoading.add(webpUrl);
    const img = new Image();
    img.decoding = 'async';
    img.onload = () => {
      mapPreviewLoading.delete(webpUrl);
      mapPreviewImageCache.set(webpUrl, img);
      const liveCanvas = document.getElementById(
        'map-radar-canvas'
      ) as HTMLCanvasElement | null;
      if (liveCanvas) {
        renderTacticalMapCanvas(liveCanvas, map);
      }
    };
    img.onerror = () => {
      mapPreviewLoading.delete(webpUrl);
    };
    img.src = webpUrl;
  }

  // Fallback tactical grid while the isometric WebP image loads
  const mapW = Math.max(16, map.width);
  const mapH = Math.max(16, map.height);
  const pad = 18;
  const drawW = w - pad * 2;
  const drawH = h - pad * 2;
  const scaleX = drawW / mapW;
  const scaleY = drawH / mapH;

  const tx = (x: number) => pad + x * scaleX;
  const ty = (y: number) => pad + y * scaleY;

  ctx.strokeStyle = 'rgba(56, 189, 248, 0.08)';
  ctx.lineWidth = 1;
  const stepTiles = Math.max(8, Math.round(mapW / 8));
  for (let gx = 0; gx <= mapW; gx += stepTiles) {
    ctx.beginPath();
    ctx.moveTo(tx(gx), pad);
    ctx.lineTo(tx(gx), pad + drawH);
    ctx.stroke();
  }
  for (let gy = 0; gy <= mapH; gy += stepTiles) {
    ctx.beginPath();
    ctx.moveTo(pad, ty(gy));
    ctx.lineTo(pad + drawW, ty(gy));
    ctx.stroke();
  }

  for (const sp of map.spawns || []) {
    const cx = tx(sp.base_tile[0]);
    const cy = ty(sp.base_tile[1]);
    ctx.fillStyle = '#38bdf8';
    ctx.beginPath();
    ctx.arc(cx, cy, 9, 0, Math.PI * 2);
    ctx.fill();
  }

  ctx.fillStyle = '#38bdf8';
  ctx.font = 'bold 12px Consolas, monospace';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(`LOADING ISOMETRIC MAP: ${map.name}...`, w * 0.5, h * 0.5);
}

interface ParsedGlbMesh {
  vertCount: number;
  triCount: number;
  byteLength: number;
  positions: Float32Array;
  indices: Uint32Array;
  triNormals: Float32Array;
  triColors: Uint8Array;
}

const glbMeshCache = new Map<string, ParsedGlbMesh>();
const glbMeshLoading = new Set<string>();
let reusableDepthBuffer: Float32Array | null = null;
let reusableScreenX: Float32Array | null = null;
let reusableScreenY: Float32Array | null = null;
let reusableScreenZ: Float32Array | null = null;

function parseGlbBinary(
  buf: ArrayBuffer,
  primaryColor: number[]
): ParsedGlbMesh | null {
  if (buf.byteLength < 28) return null;
  const dv = new DataView(buf);
  const magic = dv.getUint32(0, true);
  const version = dv.getUint32(4, true);
  if (magic !== 0x46546c67 || version !== 2) return null;

  const jsonLen = dv.getUint32(12, true);
  const jsonType = dv.getUint32(16, true);
  if (jsonType !== 0x4e4f534a || 20 + jsonLen + 8 > buf.byteLength) return null;

  const jsonText = new TextDecoder().decode(new Uint8Array(buf, 20, jsonLen));
  const gltf = JSON.parse(jsonText) as {
    accessors: Array<{
      bufferView: number;
      componentType: number;
      count: number;
    }>;
    bufferViews: Array<{
      byteOffset: number;
      byteLength: number;
    }>;
  };

  const binOffset = 20 + jsonLen + 8;
  const posAcc = gltf.accessors[0];
  const normAcc = gltf.accessors[1];
  const colAcc = gltf.accessors[2];
  const idxAcc = gltf.accessors[3];
  if (!posAcc || !normAcc || !colAcc || !idxAcc) return null;

  const posBv = gltf.bufferViews[posAcc.bufferView ?? 0];
  const normBv = gltf.bufferViews[normAcc.bufferView ?? 1];
  const colBv = gltf.bufferViews[colAcc.bufferView ?? 2];
  const idxBv = gltf.bufferViews[idxAcc.bufferView ?? 3];

  const vertCount = posAcc.count;
  const idxCount = idxAcc.count;
  const triCount = Math.floor(idxCount / 3);

  const rawPos = new Float32Array(
    buf.slice(
      binOffset + posBv.byteOffset,
      binOffset + posBv.byteOffset + vertCount * 12
    )
  );
  const rawNorm = new Float32Array(
    buf.slice(
      binOffset + normBv.byteOffset,
      binOffset + normBv.byteOffset + vertCount * 12
    )
  );
  const rawCol = new Uint8Array(
    buf,
    binOffset + colBv.byteOffset,
    vertCount * 4
  );

  let indices: Uint32Array;
  if (idxAcc.componentType === 5125) {
    indices = new Uint32Array(
      buf.slice(
        binOffset + idxBv.byteOffset,
        binOffset + idxBv.byteOffset + idxCount * 4
      )
    );
  } else {
    const u16 = new Uint16Array(
      buf.slice(
        binOffset + idxBv.byteOffset,
        binOffset + idxBv.byteOffset + idxCount * 2
      )
    );
    indices = new Uint32Array(u16.length);
    for (let i = 0; i < u16.length; i++) indices[i] = u16[i];
  }

  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;
  let minZ = Infinity;
  let maxZ = -Infinity;

  for (let i = 0; i < vertCount; i++) {
    const x = rawPos[i * 3];
    const y = rawPos[i * 3 + 1];
    const z = rawPos[i * 3 + 2];
    if (x < minX) minX = x;
    if (x > maxX) maxX = x;
    if (y < minY) minY = y;
    if (y > maxY) maxY = y;
    if (z < minZ) minZ = z;
    if (z > maxZ) maxZ = z;
  }

  const midX = (minX + maxX) * 0.5;
  const midZ = (minZ + maxZ) * 0.5;
  const spanX = maxX - minX;
  const spanY = maxY - minY;
  const spanZ = maxZ - minZ;
  const maxSpan = Math.max(0.001, spanX, spanY * 1.35, spanZ);
  const scale = 225 / maxSpan;

  const positions = new Float32Array(vertCount * 3);
  for (let i = 0; i < vertCount; i++) {
    positions[i * 3] = (rawPos[i * 3] - midX) * scale;
    positions[i * 3 + 1] = (rawPos[i * 3 + 1] - minY) * scale;
    positions[i * 3 + 2] = (rawPos[i * 3 + 2] - midZ) * scale;
  }

  const pr = Math.round((primaryColor[0] ?? 0.25) * 255);
  const pg = Math.round((primaryColor[1] ?? 0.48) * 255);
  const pb = Math.round((primaryColor[2] ?? 0.85) * 255);

  const srgbVertCols = new Uint8Array(vertCount * 3);
  for (let i = 0; i < vertCount; i++) {
    const lr = rawCol[i * 4] / 255;
    const lg = rawCol[i * 4 + 1] / 255;
    const lb = rawCol[i * 4 + 2] / 255;
    const sr = Math.pow(lr, 1 / 2.15) * 255;
    const sg = Math.pow(lg, 1 / 2.15) * 255;
    const sb = Math.pow(lb, 1 / 2.15) * 255;
    srgbVertCols[i * 3] = Math.min(255, Math.round(sr * 0.9 + pr * 0.1));
    srgbVertCols[i * 3 + 1] = Math.min(255, Math.round(sg * 0.9 + pg * 0.1));
    srgbVertCols[i * 3 + 2] = Math.min(255, Math.round(sb * 0.9 + pb * 0.1));
  }

  const triNormals = new Float32Array(triCount * 3);
  const triColors = new Uint8Array(triCount * 3);

  for (let t = 0; t < triCount; t++) {
    const i0 = indices[t * 3];
    const i1 = indices[t * 3 + 1];
    const i2 = indices[t * 3 + 2];

    const i03 = i0 * 3;
    const i13 = i1 * 3;
    const i23 = i2 * 3;

    const ax = positions[i13] - positions[i03];
    const ay = positions[i13 + 1] - positions[i03 + 1];
    const az = positions[i13 + 2] - positions[i03 + 2];
    const bx = positions[i23] - positions[i03];
    const by = positions[i23 + 1] - positions[i03 + 1];
    const bz = positions[i23 + 2] - positions[i03 + 2];

    let fnx = ay * bz - az * by;
    let fny = az * bx - ax * bz;
    let fnz = ax * by - ay * bx;
    const flen = Math.hypot(fnx, fny, fnz);
    if (flen > 1e-6) {
      fnx /= flen;
      fny /= flen;
      fnz /= flen;
    }

    const vnx = (rawNorm[i03] + rawNorm[i13] + rawNorm[i23]) / 3;
    const vny = (rawNorm[i03 + 1] + rawNorm[i13 + 1] + rawNorm[i23 + 1]) / 3;
    const vnz = (rawNorm[i03 + 2] + rawNorm[i13 + 2] + rawNorm[i23 + 2]) / 3;

    if (fnx * vnx + fny * vny + fnz * vnz < 0) {
      fnx = -fnx;
      fny = -fny;
      fnz = -fnz;
    }

    let nx = fnx * 0.6 + vnx * 0.4;
    let ny = fny * 0.6 + vny * 0.4;
    let nz = fnz * 0.6 + vnz * 0.4;
    const nlen = Math.hypot(nx, ny, nz) || 1;
    triNormals[t * 3] = nx / nlen;
    triNormals[t * 3 + 1] = ny / nlen;
    triNormals[t * 3 + 2] = nz / nlen;

    triColors[t * 3] = Math.round(
      (srgbVertCols[i03] + srgbVertCols[i13] + srgbVertCols[i23]) / 3
    );
    triColors[t * 3 + 1] = Math.round(
      (srgbVertCols[i03 + 1] + srgbVertCols[i13 + 1] + srgbVertCols[i23 + 1]) /
        3
    );
    triColors[t * 3 + 2] = Math.round(
      (srgbVertCols[i03 + 2] + srgbVertCols[i13 + 2] + srgbVertCols[i23 + 2]) /
        3
    );
  }

  return {
    vertCount,
    triCount,
    byteLength: buf.byteLength,
    positions,
    indices,
    triNormals,
    triColors,
  };
}

export function render3DTurntableCanvas(
  canvas: HTMLCanvasElement,
  item: UnitRecord | BuildingRecord,
  yawAngle: number,
  _turretAngle: number
): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const bg = ctx.createRadialGradient(
    w * 0.5,
    h * 0.58,
    12,
    w * 0.5,
    h * 0.58,
    w * 0.64
  );
  bg.addColorStop(0, '#13243b');
  bg.addColorStop(1, '#050911');
  ctx.fillStyle = bg;
  ctx.fillRect(0, 0, w, h);

  const cx = w * 0.5;
  const cy = h * 0.67;

  const cosY = Math.cos(yawAngle);
  const sinY = Math.sin(yawAngle);
  const pitch = 0.46;
  const cosP = Math.cos(pitch);
  const sinP = Math.sin(pitch);

  const project = (x: number, y: number, z: number): [number, number] => {
    const rx = x * cosY - z * sinY;
    const rz = x * sinY + z * cosY;
    const sx = cx + rx;
    const sy = cy - y * cosP + rz * sinP;
    return [sx, sy];
  };

  // Turntable outer ring & calibration ticks
  ctx.strokeStyle = 'rgba(56, 189, 248, 0.32)';
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  for (let a = 0; a <= Math.PI * 2 + 0.05; a += 0.12) {
    const [px, py] = project(Math.cos(a) * 112, 0, Math.sin(a) * 112);
    if (a === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  }
  ctx.stroke();

  ctx.strokeStyle = 'rgba(148, 163, 184, 0.14)';
  ctx.lineWidth = 1;
  for (let g = -75; g <= 75; g += 25) {
    const [x1, y1] = project(g, 0, -78);
    const [x2, y2] = project(g, 0, 78);
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();

    const [z1, w1] = project(-78, 0, g);
    const [z2, w2] = project(78, 0, g);
    ctx.beginPath();
    ctx.moveTo(z1, w1);
    ctx.lineTo(z2, w2);
    ctx.stroke();
  }

  // Soft ground shadow under model
  ctx.fillStyle = 'rgba(2, 6, 12, 0.55)';
  ctx.beginPath();
  for (let a = 0; a <= Math.PI * 2 + 0.05; a += 0.15) {
    const [px, py] = project(Math.cos(a) * 68, 0, Math.sin(a) * 68);
    if (a === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  }
  ctx.fill();

  const rawModelPath = item.visual?.model || `assets/models/${item.id}.glb`;
  const modelUrl = '/' + rawModelPath.replace(/^\/+/, '');
  const mesh = glbMeshCache.get(modelUrl);

  if (!mesh) {
    if (!glbMeshLoading.has(modelUrl) && typeof fetch !== 'undefined') {
      glbMeshLoading.add(modelUrl);
      fetch(modelUrl)
        .then((res) => (res.ok ? res.arrayBuffer() : null))
        .then((buf) => {
          glbMeshLoading.delete(modelUrl);
          if (!buf) return;
          const parsed = parseGlbBinary(
            buf,
            item.visual?.primary_color || [0.22, 0.45, 0.88]
          );
          if (parsed) {
            glbMeshCache.set(modelUrl, parsed);
            const liveCanvas = document.getElementById(
              'inspector-3d-canvas'
            ) as HTMLCanvasElement | null;
            if (liveCanvas) {
              render3DTurntableCanvas(liveCanvas, item, yawAngle, 0);
            }
          }
        })
        .catch(() => {
          glbMeshLoading.delete(modelUrl);
        });
    }

    ctx.fillStyle = '#38bdf8';
    ctx.font = 'bold 11px Consolas, monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(`STREAMING GLB 2.0 MESH: ${modelUrl}...`, w * 0.5, h * 0.45);
    return;
  }

  const isAir =
    (item as UnitRecord).movement_domain === 'air' ||
    (item.visual?.archetype || '').toLowerCase().includes('aircraft') ||
    (item.visual?.archetype || '').toLowerCase().includes('heli');
  const yLift = isAir ? 16 : 0;

  const vertCount = mesh.vertCount;
  if (!reusableScreenX || reusableScreenX.length < vertCount) {
    reusableScreenX = new Float32Array(vertCount + 1024);
    reusableScreenY = new Float32Array(vertCount + 1024);
    reusableScreenZ = new Float32Array(vertCount + 1024);
  }
  const sxArr = reusableScreenX;
  const syArr = reusableScreenY!;
  const szArr = reusableScreenZ!;
  const pos = mesh.positions;

  for (let v = 0; v < vertCount; v++) {
    const v3 = v * 3;
    const wx = pos[v3];
    const wy = pos[v3 + 1] + yLift;
    const wz = pos[v3 + 2];

    const rx = wx * cosY - wz * sinY;
    const rz = wx * sinY + wz * cosY;

    sxArr[v] = cx + rx;
    syArr[v] = cy - wy * cosP + rz * sinP;
    szArr[v] = rz * cosP + wy * sinP;
  }

  const pixelCount = w * h;
  if (!reusableDepthBuffer || reusableDepthBuffer.length !== pixelCount) {
    reusableDepthBuffer = new Float32Array(pixelCount);
  }
  const depthBuf = reusableDepthBuffer;
  depthBuf.fill(-1e9);

  const imgData = ctx.getImageData(0, 0, w, h);
  const pix = imgData.data;

  const idxs = mesh.indices;
  const tNorms = mesh.triNormals;
  const tCols = mesh.triColors;
  const triCount = mesh.triCount;

  for (let t = 0; t < triCount; t++) {
    const t3 = t * 3;
    const i0 = idxs[t3];
    const i1 = idxs[t3 + 1];
    const i2 = idxs[t3 + 2];

    let x0 = sxArr[i0];
    let y0 = syArr[i0];
    let z0 = szArr[i0];
    let x1 = sxArr[i1];
    let y1 = syArr[i1];
    let z1 = szArr[i1];
    let x2 = sxArr[i2];
    let y2 = syArr[i2];
    let z2 = szArr[i2];

    let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    if (area > -0.04 && area < 0.04) continue;
    if (area < 0) {
      const tx = x1;
      const ty = y1;
      const tz = z1;
      x1 = x2;
      y1 = y2;
      z1 = z2;
      x2 = tx;
      y2 = ty;
      z2 = tz;
      area = -area;
    }

    const minX = Math.max(0, Math.floor(Math.min(x0, x1, x2)));
    const maxX = Math.min(w - 1, Math.ceil(Math.max(x0, x1, x2)));
    const minY = Math.max(0, Math.floor(Math.min(y0, y1, y2)));
    const maxY = Math.min(h - 1, Math.ceil(Math.max(y0, y1, y2)));
    if (minX > maxX || minY > maxY) continue;

    const mnx = tNorms[t3];
    const mny = tNorms[t3 + 1];
    const mnz = tNorms[t3 + 2];

    const rnx = mnx * cosY - mnz * sinY;
    const rnz = mnx * sinY + mnz * cosY;
    const vny = mny * cosP - rnz * sinP;
    const vnz = rnz * cosP + mny * sinP;

    const sign = vnz < 0 ? -1 : 1;
    const nx = rnx * sign;
    const ny = vny * sign;
    const nz = vnz * sign;

    const keyDiff = Math.max(0, nx * 0.52 + ny * 0.7 + nz * 0.48);
    const fillDiff = Math.max(0, -nx * 0.65 + ny * 0.3 - nz * 0.68);
    const hemi = 0.42 + 0.18 * (ny * 0.5 + 0.5);
    const spec =
      Math.pow(Math.max(0, nx * 0.28 + ny * 0.44 + nz * 0.85), 14) * 34;
    const light = hemi + keyDiff * 0.68 + fillDiff * 0.22;

    const rLit = Math.min(255, Math.round(tCols[t3] * light + spec));
    const gLit = Math.min(255, Math.round(tCols[t3 + 1] * light + spec));
    const bLit = Math.min(255, Math.round(tCols[t3 + 2] * light + spec * 1.1));

    const invArea = 1.0 / area;
    for (let py = minY; py <= maxY; py++) {
      const cySample = py + 0.5;
      const rowOffset = py * w;
      for (let px = minX; px <= maxX; px++) {
        const cxSample = px + 0.5;
        const w0 =
          ((x1 - cxSample) * (y2 - cySample) -
            (x2 - cxSample) * (y1 - cySample)) *
          invArea;
        if (w0 < 0) continue;
        const w1 =
          ((x2 - cxSample) * (y0 - cySample) -
            (x0 - cxSample) * (y2 - cySample)) *
          invArea;
        if (w1 < 0) continue;
        const w2 = 1.0 - w0 - w1;
        if (w2 < 0) continue;

        const z = w0 * z0 + w1 * z1 + w2 * z2;
        const idx = rowOffset + px;
        if (z > depthBuf[idx]) {
          depthBuf[idx] = z;
          const pIdx = idx << 2;
          pix[pIdx] = rLit;
          pix[pIdx + 1] = gLit;
          pix[pIdx + 2] = bLit;
          pix[pIdx + 3] = 255;
        }
      }
    }
  }

  ctx.putImageData(imgData, 0, 0);

  // Top telemetry badge inside 3D viewport
  ctx.fillStyle = 'rgba(5, 9, 18, 0.78)';
  ctx.fillRect(6, 6, w - 12, 20);
  ctx.font = 'bold 10px Consolas, monospace';
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'left';
  ctx.fillStyle = '#38bdf8';
  ctx.fillText(
    `GLB 2.0 MESH // ${mesh.vertCount.toLocaleString()} VERTS · ${mesh.triCount.toLocaleString()} TRIS`,
    12,
    16
  );
  ctx.textAlign = 'right';
  ctx.fillStyle = '#94a3b8';
  ctx.fillText(
    `${modelUrl.split('/').pop()} (${(mesh.byteLength / 1024).toFixed(1)} KB)`,
    w - 12,
    16
  );
}

export function renderModsGridHtml(
  mods: ModSummary[],
  activeMasterUrl: string,
  lang: Locale = 'de'
): string {
  return mods
    .map((mod) => {
      const sampleMaps = mod.maps
        .slice(0, 5)
        .map((mp) => `<span class="map-pill">${escapeHtml(mp.name)}</span>`)
        .join('');
      const moreMaps =
        mod.maps.length > 5
          ? `<span class="map-pill map-pill-more">+${mod.maps.length - 5} ${lang === 'de' ? 'weitere' : 'more'}</span>`
          : '';
      const scriptsBadges =
        mod.scripts.length > 0
          ? mod.scripts
              .map((s) => `<code class="script-badge">${escapeHtml(s)}</code>`)
              .join(' ')
          : `<span class="cell-sub">${lang === 'de' ? 'Deklarative TOML-Regeln' : 'Declarative TOML rules'}</span>`;

      const modPathArg =
        mod.id === 'rust-ra2'
          ? 'rust-ra2'
          : `mods/${mod.id.replace('_', '-')}`;
      const launchCmd = `cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path ${modPathArg} --lobby-server ${activeMasterUrl}`;
      const desc =
        lang === 'de' && MOD_DESC_DE[mod.id]
          ? MOD_DESC_DE[mod.id]
          : mod.description;

      return `
        <article class="mod-card">
          <div class="mod-card-header">
            <div>
              <span class="tag tag-mod">${escapeHtml(mod.id)}</span>
              <span class="version-label">v${escapeHtml(mod.version)}</span>
              <h3 class="mod-card-title">${escapeHtml(mod.name)}</h3>
            </div>
            <button
              type="button"
              class="btn btn-xs btn-secondary"
              data-filter-mod="${escapeHtml(mod.id)}"
            >
              ${lang === 'de' ? 'Lobbys filtern' : 'Filter Lobbies'}
            </button>
          </div>
          <p class="mod-card-desc">${escapeHtml(desc)}</p>
          <dl class="mod-metrics">
            <div>
              <dt>${lang === 'de' ? 'Fraktionen' : 'Factions'}</dt>
              <dd>${mod.factions_count}</dd>
            </div>
            <div>
              <dt>${lang === 'de' ? 'Einheiten' : 'Units'}</dt>
              <dd>${mod.units_count}</dd>
            </div>
            <div>
              <dt>${lang === 'de' ? 'Gebäude' : 'Buildings'}</dt>
              <dd>${mod.buildings_count}</dd>
            </div>
            <div>
              <dt>${lang === 'de' ? 'Karten' : 'Maps'}</dt>
              <dd>${mod.maps.length}</dd>
            </div>
          </dl>
          <div class="mod-meta-block">
            <div class="meta-label">${lang === 'de' ? 'Skript-Hooks (Rhai / Lua):' : 'Script Hooks (Rhai / Lua):'}</div>
            <div class="meta-content">${scriptsBadges}</div>
          </div>
          <div class="mod-meta-block">
            <div class="meta-label">${lang === 'de' ? 'Verfügbare Karten' : 'Available Maps'} (${mod.maps.length}):</div>
            <div class="map-pills">${sampleMaps}${moreMaps}</div>
          </div>
          <div class="mod-card-footer">
            <code class="cli-inline" title="${escapeHtml(launchCmd)}">${escapeHtml(launchCmd)}</code>
            <button
              type="button"
              class="btn btn-xs btn-ghost"
              data-copy="${escapeHtml(launchCmd)}"
            >
              ${escapeHtml(t(lang, 'btn_copy'))}
            </button>
          </div>
        </article>
      `;
    })
    .join('');
}

export function renderCratesGridHtml(
  crates: EngineCrateInfo[],
  lang: Locale = 'de'
): string {
  return crates
    .map((c) => {
      const enOverride = lang === 'en' ? CRATES_EN[c.name] : undefined;
      const role = enOverride ? enOverride.role : c.role;
      const highlights = enOverride ? enOverride.highlights : c.highlights;

      return `
      <article class="crate-card">
        <div class="crate-header">
          <code class="crate-name">${escapeHtml(c.name)}</code>
          <span class="tag tag-command">${lang === 'de' ? 'Engine Crate' : 'Engine Crate'}</span>
        </div>
        <p class="crate-role">${escapeHtml(role)}</p>
        <ul class="crate-highlights">
          ${highlights.map((h) => `<li>${escapeHtml(h)}</li>`).join('')}
        </ul>
      </article>
    `;
    })
    .join('');
}

export function updateTelemetryDom(
  stats: RegistryStats,
  lang: Locale = 'de'
): void {
  const setText = (id: string, text: string) => {
    const el = document.getElementById(id);
    if (el) el.textContent = text;
  };

  setText('stat-active-lobbies', String(stats.active_lobbies));
  setText(
    'stat-open-lobbies',
    lang === 'de'
      ? `${stats.open_lobbies} offen · ${stats.in_game_lobbies} im Spiel`
      : `${stats.open_lobbies} open · ${stats.in_game_lobbies} in game`
  );
  setText('stat-players-online', String(stats.players_online));
  setText('stat-matches-total', String(stats.total_matches_completed));
  setText('stat-ttl-secs', `${stats.heartbeat_ttl_secs}s Heartbeat-TTL`);
  setText(
    'stat-uptime',
    `Node: ${stats.node_id} · Uptime: ${formatDuration(stats.uptime_secs)}`
  );
}
