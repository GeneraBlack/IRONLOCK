import {
  fetchGameDatabase,
  fetchLeaderboard,
  fetchLobbies,
  fetchMatches,
  fetchProjectInfo,
  fetchStats,
} from './api.js';
import {
  applyStaticDomTranslations,
  detectInitialLocale,
  Locale,
  t,
} from './i18n.js';
import {
  escapeHtml,
  factionCssClass,
  factionDisplayName,
  render3DTurntableCanvas,
  renderArsenalGridHtml,
  renderArsenalInspectorHtml,
  renderCountriesStripHtml,
  renderCountryDetailHtml,
  renderCratesGridHtml,
  renderHeroCrateDetailHtml,
  renderLeaderboardHtml,
  renderLobbyRowsHtml,
  renderMatchesRowsHtml,
  renderModsGridHtml,
  renderPipelineSpecTabHtml,
  renderTacticalMapCanvas,
  updateTelemetryDom,
} from './render.js';
import {
  BuildingRecord,
  CommanderRankEntry,
  GameDatabase,
  LobbyEntry,
  LobbyStatus,
  MapRadarRecord,
  MatchRecord,
  ProjectOverview,
  RegistryStats,
  UnitRecord,
} from './types.js';

type PipelineTab = 'schema' | 'scripting' | 'assets' | 'network';
type InspectorMode = 'entities' | 'maps' | 'factions';
type NetworkTab = 'lobbies' | 'matches' | 'leaderboard';

interface AppState {
  lang: Locale;
  lobbies: LobbyEntry[];
  matches: MatchRecord[];
  leaderboard: CommanderRankEntry[];
  lastStats: RegistryStats | null;
  projectInfo: ProjectOverview | null;
  gameDb: GameDatabase | null;
  ttlSecs: number;
  modFilter: string;
  statusFilter: LobbyStatus | '';
  onlyOpen: boolean;
  searchQuery: string;
  matchModFilter: string;
  matchSearchQuery: string;
  autoRefresh: boolean;
  selectedCrateId: string;
  pipelineTab: PipelineTab;
  networkTab: NetworkTab;
  inspectorMode: InspectorMode;
  selectedCountryId: string;
  arsenalKind: 'units' | 'buildings';
  arsenalFaction: string;
  arsenalRole: string;
  arsenalSearch: string;
  selectedArsenalId: string;
  mapCategory: string;
  mapSearch: string;
  selectedMapId: string;
  turntableYaw: number;
  turntableTurret: number;
  turntableDragging: boolean;
  turntableLastX: number;
}

const state: AppState = {
  lang: detectInitialLocale(),
  lobbies: [],
  matches: [],
  leaderboard: [],
  lastStats: null,
  projectInfo: null,
  gameDb: null,
  ttlSecs: 45,
  modFilter: '',
  statusFilter: '',
  onlyOpen: false,
  searchQuery: '',
  matchModFilter: '',
  matchSearchQuery: '',
  autoRefresh: true,
  selectedCrateId: 'rts-sdk',
  pipelineTab: 'schema',
  networkTab: 'lobbies',
  inspectorMode: 'entities',
  selectedCountryId: 'usa',
  arsenalKind: 'units',
  arsenalFaction: 'all',
  arsenalRole: 'all',
  arsenalSearch: '',
  selectedArsenalId: 'grizzly_tank',
  mapCategory: 'skirmish',
  mapSearch: '',
  selectedMapId: 'mp_01_offense_defense',
  turntableYaw: 0.65,
  turntableTurret: 0.25,
  turntableDragging: false,
  turntableLastX: 0,
};

function showToast(message: string, variant: 'info' | 'error' = 'info'): void {
  const region = document.getElementById('toast-region');
  if (!region) return;
  const toast = document.createElement('div');
  toast.className = `toast toast-${variant}`;
  toast.textContent = message;
  region.appendChild(toast);
  window.setTimeout(() => {
    toast.classList.add('toast-hide');
    window.setTimeout(() => toast.remove(), 250);
  }, 3200);
}

async function copyToClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    showToast(`${t(state.lang, 'toast_copied')}: ${text}`);
  } catch {
    showToast(t(state.lang, 'toast_copy_err'), 'error');
  }
}

function setServerBadge(online: boolean): void {
  const badge = document.getElementById('server-status-badge');
  if (!badge) return;
  if (online) {
    badge.className = 'server-badge badge-online';
    badge.textContent = t(state.lang, 'server_online');
  } else {
    badge.className = 'server-badge badge-offline';
    badge.textContent = t(state.lang, 'server_offline');
  }
}

function renderHeroCrateInspector(): void {
  document.querySelectorAll('[data-select-crate]').forEach((el) => {
    const cid = el.getAttribute('data-select-crate');
    const isActive = cid === state.selectedCrateId;
    el.classList.toggle('crate-tab-active', isActive);
    el.setAttribute('aria-pressed', String(isActive));
  });

  const detailEl = document.getElementById('hero-crate-detail');
  if (detailEl) {
    detailEl.innerHTML = renderHeroCrateDetailHtml(
      state.selectedCrateId,
      state.lang
    );
  }
}

function renderPipelineSpecSection(): void {
  document.querySelectorAll('[data-pipeline-tab]').forEach((el) => {
    const tab = el.getAttribute('data-pipeline-tab');
    const isActive = tab === state.pipelineTab;
    el.classList.toggle('tab-btn-active', isActive);
    el.setAttribute('aria-pressed', String(isActive));
  });

  const stageEl = document.getElementById('pipeline-spec-stage');
  if (stageEl) {
    stageEl.innerHTML = renderPipelineSpecTabHtml(
      state.pipelineTab,
      state.lang
    );
  }
}

function renderNetworkTabSwitch(): void {
  document.querySelectorAll('[data-network-tab]').forEach((el) => {
    const tab = el.getAttribute('data-network-tab');
    const isActive = tab === state.networkTab;
    el.classList.toggle('mode-switch-active', isActive);
    el.setAttribute('aria-pressed', String(isActive));
  });

  const paneLobbies = document.getElementById('network-pane-lobbies');
  const paneMatches = document.getElementById('network-pane-matches');
  const paneLeaderboard = document.getElementById('network-pane-leaderboard');

  if (paneLobbies) paneLobbies.hidden = state.networkTab !== 'lobbies';
  if (paneMatches) paneMatches.hidden = state.networkTab !== 'matches';
  if (paneLeaderboard) {
    paneLeaderboard.hidden = state.networkTab !== 'leaderboard';
  }
}

function renderInspectorModeSwitch(): void {
  document.querySelectorAll('[data-inspector-mode]').forEach((el) => {
    const mode = el.getAttribute('data-inspector-mode');
    const isActive = mode === state.inspectorMode;
    el.classList.toggle('mode-switch-active', isActive);
    el.setAttribute('aria-pressed', String(isActive));
  });

  const paneEntities = document.getElementById('inspector-pane-entities');
  const paneMaps = document.getElementById('inspector-pane-maps');
  const paneFactions = document.getElementById('inspector-pane-factions');

  if (paneEntities) paneEntities.hidden = state.inspectorMode !== 'entities';
  if (paneMaps) paneMaps.hidden = state.inspectorMode !== 'maps';
  if (paneFactions) paneFactions.hidden = state.inspectorMode !== 'factions';

  if (state.inspectorMode === 'maps') {
    renderMapsAtlasSection();
  }
}

function renderCountriesSection(): void {
  if (!state.gameDb) return;
  const stripEl = document.getElementById('countries-strip');
  const detailEl = document.getElementById('country-detail-panel');
  if (!stripEl || !detailEl) return;

  stripEl.innerHTML = renderCountriesStripHtml(
    state.gameDb.countries,
    state.selectedCountryId,
    state.lang
  );

  const activeCountry =
    state.gameDb.countries.find((c) => c.id === state.selectedCountryId) ||
    state.gameDb.countries[0];
  if (activeCountry) {
    detailEl.innerHTML = renderCountryDetailHtml(activeCountry, state.lang);
  }
}

function getFilteredArsenalItems(): Array<UnitRecord | BuildingRecord> {
  if (!state.gameDb) return [];
  const rawList: Array<UnitRecord | BuildingRecord> =
    state.arsenalKind === 'units'
      ? state.gameDb.units
      : state.gameDb.buildings;

  const q = state.arsenalSearch.trim().toLowerCase();
  return rawList.filter((item) => {
    if (state.arsenalFaction !== 'all' && item.faction !== state.arsenalFaction) {
      return false;
    }
    if (state.arsenalRole !== 'all') {
      if (state.arsenalKind === 'units') {
        const u = item as UnitRecord;
        if (state.arsenalRole === 'air') {
          if (u.movement_domain !== 'air' && u.movement_domain !== 'jumpjet') {
            return false;
          }
        } else if (state.arsenalRole === 'naval') {
          if (u.movement_domain !== 'naval') return false;
        } else if (u.role !== state.arsenalRole) {
          return false;
        }
      } else {
        if (item.role !== state.arsenalRole) return false;
      }
    }
    if (q.length > 0) {
      const hay = `${item.name} ${item.id} ${item.role} ${item.faction}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
}

function getSelectedArsenalEntity(): UnitRecord | BuildingRecord | null {
  if (!state.gameDb) return null;
  const list: Array<UnitRecord | BuildingRecord> =
    state.arsenalKind === 'units'
      ? state.gameDb.units
      : state.gameDb.buildings;
  return (
    list.find((x) => x.id === state.selectedArsenalId) || list[0] || null
  );
}

function renderArsenalSection(): void {
  if (!state.gameDb) return;

  const btnUnits = document.getElementById('btn-arsenal-units');
  const btnBuildings = document.getElementById('btn-arsenal-buildings');
  if (btnUnits && btnBuildings) {
    btnUnits.classList.toggle('tab-btn-active', state.arsenalKind === 'units');
    btnBuildings.classList.toggle(
      'tab-btn-active',
      state.arsenalKind === 'buildings'
    );
  }

  document.querySelectorAll('[data-arsenal-faction]').forEach((el) => {
    const fac = el.getAttribute('data-arsenal-faction');
    el.classList.toggle('pill-btn-active', fac === state.arsenalFaction);
  });

  const filtered = getFilteredArsenalItems();
  if (
    filtered.length > 0 &&
    !filtered.some((x) => x.id === state.selectedArsenalId)
  ) {
    state.selectedArsenalId = filtered[0].id;
  }

  const countEl = document.getElementById('arsenal-results-count');
  if (countEl) {
    if (state.lang === 'de') {
      countEl.textContent = `${filtered.length} ${state.arsenalKind === 'units' ? 'Einheit(en)' : 'Gebäude'} angezeigt`;
    } else {
      countEl.textContent = `${filtered.length} ${state.arsenalKind === 'units' ? 'unit(s)' : 'structure(s)'} shown`;
    }
  }

  const gridEl = document.getElementById('arsenal-cameo-grid');
  if (gridEl) {
    gridEl.innerHTML = renderArsenalGridHtml(
      filtered,
      state.selectedArsenalId,
      state.arsenalKind,
      state.lang
    );
  }

  renderArsenalInspectorPanel();
}

function renderArsenalInspectorPanel(): void {
  if (!state.gameDb) return;
  const inspectorEl = document.getElementById('arsenal-inspector-panel');
  if (!inspectorEl) return;

  const entity = getSelectedArsenalEntity();
  if (!entity) return;

  inspectorEl.innerHTML = renderArsenalInspectorHtml(
    entity,
    state.arsenalKind,
    state.gameDb,
    state.lang
  );

  const canvas = document.getElementById(
    'inspector-3d-canvas'
  ) as HTMLCanvasElement | null;
  if (canvas) {
    render3DTurntableCanvas(
      canvas,
      entity,
      state.turntableYaw,
      state.turntableTurret
    );

    canvas.addEventListener('pointerdown', (ev) => {
      state.turntableDragging = true;
      state.turntableLastX = ev.clientX;
      canvas.setPointerCapture(ev.pointerId);
    });
    canvas.addEventListener('pointermove', (ev) => {
      if (!state.turntableDragging) return;
      const dx = ev.clientX - state.turntableLastX;
      state.turntableLastX = ev.clientX;
      state.turntableYaw += dx * 0.014;
      render3DTurntableCanvas(
        canvas,
        entity,
        state.turntableYaw,
        state.turntableTurret
      );
    });
    canvas.addEventListener('pointerup', () => {
      state.turntableDragging = false;
    });
  }
}

function getFilteredMaps(): MapRadarRecord[] {
  if (!state.gameDb) return [];
  const q = state.mapSearch.trim().toLowerCase();
  return state.gameDb.maps.filter((m) => {
    if (state.mapCategory !== 'all') {
      if (state.mapCategory === 'campaign_yr') {
        if (!m.category.startsWith('campaign_yr')) return false;
      } else if (m.category !== state.mapCategory) {
        return false;
      }
    }
    if (q.length > 0) {
      const hay = `${m.name} ${m.id} ${m.mission_objective}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
}

function renderMapsAtlasSection(): void {
  if (!state.gameDb) return;

  document.querySelectorAll('[data-map-category]').forEach((el) => {
    const cat = el.getAttribute('data-map-category');
    el.classList.toggle('tab-btn-active', cat === state.mapCategory);
  });

  const filtered = getFilteredMaps();
  if (
    filtered.length > 0 &&
    !filtered.some((m) => m.id === state.selectedMapId)
  ) {
    state.selectedMapId = filtered[0].id;
  }

  const listEl = document.getElementById('maps-selector-list');
  if (listEl) {
    listEl.innerHTML = filtered
      .map((m) => {
        const isSelected = m.id === state.selectedMapId;
        return `
          <button
            type="button"
            class="map-list-btn ${isSelected ? 'map-list-btn-active' : ''}"
            data-select-map="${escapeHtml(m.id)}"
            aria-pressed="${isSelected}"
          >
            <div class="map-list-title">
              <strong>${escapeHtml(m.name)}</strong>
              <span><code>${escapeHtml(m.id)}</code> · ${m.width}×${m.height} Tiles</span>
            </div>
            <span class="tag tag-mod">${m.max_players}P</span>
          </button>
        `;
      })
      .join('');
  }

  const activeMap =
    state.gameDb.maps.find((m) => m.id === state.selectedMapId) || filtered[0];
  if (!activeMap) return;

  const canvas = document.getElementById(
    'map-radar-canvas'
  ) as HTMLCanvasElement | null;
  if (canvas) {
    renderTacticalMapCanvas(canvas, activeMap);
  }

  const detailEl = document.getElementById('map-radar-telemetry');
  if (detailEl) {
    const spawnsHtml = activeMap.spawns
      .map(
        (sp) => `
        <span class="faction-badge ${factionCssClass(sp.faction)}">
          P${sp.player_id}: ${escapeHtml(factionDisplayName(sp.faction, state.lang))} (${sp.base_tile[0]}, ${sp.base_tile[1]})
        </span>
      `
      )
      .join(' ');

    const totalOrePatches = activeMap.ore_patches.length;
    const totalTechBldgs =
      activeMap.structures_count ?? activeMap.tech_structures.length;
    const theaterName = (activeMap.theater || 'TEMPERATE').toUpperCase();
    const hostMapCmd = `cargo run --manifest-path engine/Cargo.toml -p rts-client -- --mod-path rust-ra2 --map ${activeMap.id} --host 0.0.0.0:7878 --lobby-server https://rust-rts.bls-isp.net`;

    detailEl.innerHTML = `
      <div class="map-dossier-header">
        <div>
          <span class="section-kicker">${escapeHtml(t(state.lang, 'map_telemetry_kicker'))} // ${escapeHtml(theaterName)}</span>
          <h3>${escapeHtml(activeMap.name)}</h3>
          <div class="cell-sub">${escapeHtml(t(state.lang, 'map_id_label'))}: <code>${escapeHtml(activeMap.id)}</code> · <code>${escapeHtml(activeMap.preview_webp || `/assets/maps/${activeMap.id}.webp`)}</code></div>
        </div>
        <button
          type="button"
          class="btn btn-xs btn-secondary"
          data-copy="${escapeHtml(hostMapCmd)}"
        >
          ${escapeHtml(t(state.lang, 'map_copy_host_btn'))}
        </button>
      </div>
      <div class="inspector-kpi-grid">
        <div class="inspector-kpi">
          <span>${escapeHtml(t(state.lang, 'map_kpi_size'))}</span>
          <strong>${activeMap.width} × ${activeMap.height} Tiles</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(t(state.lang, 'map_kpi_extent'))}</span>
          <strong>${Math.round(activeMap.width * activeMap.tile_size)} × ${Math.round(activeMap.height * activeMap.tile_size)} m</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(t(state.lang, 'map_kpi_spawns'))}</span>
          <strong>${activeMap.max_players} ${escapeHtml(t(state.lang, 'map_kpi_commanders'))}</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(t(state.lang, 'map_kpi_ore'))}</span>
          <strong>${totalOrePatches} ${escapeHtml(t(state.lang, 'map_kpi_deposits'))}</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(t(state.lang, 'map_kpi_regen'))}</span>
          <strong>+${activeMap.ore_regen_per_sec}/s</strong>
        </div>
        <div class="inspector-kpi">
          <span>${escapeHtml(t(state.lang, 'map_kpi_tech'))}</span>
          <strong>${totalTechBldgs} ${escapeHtml(t(state.lang, 'map_kpi_structures'))}</strong>
        </div>
      </div>
      <div class="inspector-section">
        <div class="inspector-section-title">${escapeHtml(t(state.lang, 'map_spawns_title'))}</div>
        <div class="spawns-badge-row">${spawnsHtml}</div>
      </div>
      <div class="radar-legend-row">
        <span class="legend-item"><i class="legend-dot legend-spawn"></i> ${escapeHtml(t(state.lang, 'legend_allied'))} / ${escapeHtml(t(state.lang, 'legend_soviet'))}</span>
        <span class="legend-item"><i class="legend-dot legend-ore"></i> ${escapeHtml(t(state.lang, 'legend_ore'))}</span>
        <span class="legend-item"><i class="legend-dot legend-gem"></i> ${escapeHtml(t(state.lang, 'legend_gems'))}</span>
        <span class="legend-item"><i class="legend-dot legend-tech"></i> ${escapeHtml(t(state.lang, 'legend_tech'))}</span>
      </div>
    `;
  }
}

function renderModsAndCratesSection(): void {
  if (!state.projectInfo) return;
  const modsGrid = document.getElementById('mods-showcase-grid');
  if (modsGrid) {
    modsGrid.innerHTML = renderModsGridHtml(
      state.projectInfo.mods,
      'https://rust-rts.bls-isp.net',
      state.lang
    );
  }

  const cratesGrid = document.getElementById('crates-showcase-grid');
  if (cratesGrid) {
    cratesGrid.innerHTML = renderCratesGridHtml(
      state.projectInfo.crates,
      state.lang
    );
  }
}

function renderLiveTablesFromState(): void {
  if (state.lastStats) {
    updateTelemetryDom(state.lastStats, state.lang);
  }

  const lobbyRows = document.getElementById('lobby-table-body');
  if (lobbyRows) {
    lobbyRows.innerHTML = renderLobbyRowsHtml(
      state.lobbies,
      state.ttlSecs,
      Date.now(),
      state.lang
    );
  }

  const lobbyCountLabel = document.getElementById('lobby-results-count');
  if (lobbyCountLabel) {
    lobbyCountLabel.textContent =
      state.lang === 'de'
        ? `${state.lobbies.length} aktive Spielsitzung(en)`
        : `${state.lobbies.length} active session(s)`;
  }

  const leaderboardRows = document.getElementById('leaderboard-table-body');
  if (leaderboardRows) {
    leaderboardRows.innerHTML = renderLeaderboardHtml(
      state.leaderboard,
      Date.now(),
      state.lang
    );
  }

  const matchRows = document.getElementById('matches-table-body');
  if (matchRows) {
    matchRows.innerHTML = renderMatchesRowsHtml(
      state.matches,
      Date.now(),
      state.lang
    );
  }
}

function setLocale(newLang: Locale): void {
  state.lang = newLang;
  try {
    window.localStorage.setItem('ironlock_lang', newLang);
  } catch {
    // ignore storage errors
  }
  applyStaticDomTranslations(newLang);
  renderHeroCrateInspector();
  renderPipelineSpecSection();
  renderInspectorModeSwitch();
  renderCountriesSection();
  renderArsenalSection();
  renderMapsAtlasSection();
  renderModsAndCratesSection();
  renderLiveTablesFromState();
}

export async function refreshAllData(): Promise<void> {
  try {
    const [stats, lobbies, matches, leaderboard] = await Promise.all([
      fetchStats(),
      fetchLobbies({
        modId: state.modFilter || undefined,
        onlyOpen: state.onlyOpen,
        status: state.statusFilter,
        search: state.searchQuery || undefined,
      }),
      fetchMatches(
        state.matchModFilter || undefined,
        state.matchSearchQuery || undefined
      ),
      fetchLeaderboard(),
    ]);

    state.lastStats = stats;
    state.lobbies = lobbies;
    state.matches = matches;
    state.leaderboard = leaderboard;
    state.ttlSecs = stats.heartbeat_ttl_secs;

    setServerBadge(true);
    renderLiveTablesFromState();
  } catch (err) {
    setServerBadge(false);
    const msg = err instanceof Error ? err.message : String(err);
    const lobbyCountLabel = document.getElementById('lobby-results-count');
    if (lobbyCountLabel) {
      lobbyCountLabel.textContent =
        state.lang === 'de'
          ? `Verbindungsfehler: ${msg}`
          : `Connection error: ${msg}`;
    }
  }
}

async function initGamePortalData(): Promise<void> {
  try {
    const [info, db] = await Promise.all([
      fetchProjectInfo(),
      fetchGameDatabase(),
    ]);
    state.projectInfo = info;
    state.gameDb = db;

    renderHeroCrateInspector();
    renderPipelineSpecSection();
    renderInspectorModeSwitch();
    renderCountriesSection();
    renderArsenalSection();
    renderMapsAtlasSection();
    renderModsAndCratesSection();

    populateModSelects(info);
  } catch {
    // non-fatal on initial load
  }
}

function populateModSelects(info: ProjectOverview): void {
  const filterSelect = document.getElementById(
    'filter-mod-select'
  ) as HTMLSelectElement | null;
  const matchModSelect = document.getElementById(
    'match-filter-mod'
  ) as HTMLSelectElement | null;

  if (filterSelect && filterSelect.options.length <= 1) {
    for (const mod of info.mods) {
      const opt = document.createElement('option');
      opt.value = mod.id;
      opt.textContent = `${mod.name} (${mod.id})`;
      filterSelect.appendChild(opt);
    }
  }

  if (matchModSelect && matchModSelect.options.length <= 1) {
    for (const mod of info.mods) {
      const opt = document.createElement('option');
      opt.value = mod.id;
      opt.textContent = `${mod.name} (${mod.id})`;
      matchModSelect.appendChild(opt);
    }
  }
}

function bindEvents(): void {
  document
    .getElementById('btn-arsenal-units')
    ?.addEventListener('click', () => {
      state.arsenalKind = 'units';
      state.arsenalRole = 'all';
      state.selectedArsenalId = 'grizzly_tank';
      const roleSel = document.getElementById(
        'arsenal-role-select'
      ) as HTMLSelectElement | null;
      if (roleSel) roleSel.value = 'all';
      renderArsenalSection();
    });

  document
    .getElementById('btn-arsenal-buildings')
    ?.addEventListener('click', () => {
      state.arsenalKind = 'buildings';
      state.arsenalRole = 'all';
      state.selectedArsenalId = 'allied_conyard';
      const roleSel = document.getElementById(
        'arsenal-role-select'
      ) as HTMLSelectElement | null;
      if (roleSel) roleSel.value = 'all';
      renderArsenalSection();
    });

  const arsenalRoleSelect = document.getElementById(
    'arsenal-role-select'
  ) as HTMLSelectElement | null;
  arsenalRoleSelect?.addEventListener('change', () => {
    state.arsenalRole = arsenalRoleSelect.value;
    renderArsenalSection();
  });

  const arsenalSearchInput = document.getElementById(
    'arsenal-search-input'
  ) as HTMLInputElement | null;
  arsenalSearchInput?.addEventListener('input', () => {
    state.arsenalSearch = arsenalSearchInput.value;
    renderArsenalSection();
  });

  const mapSearchInput = document.getElementById(
    'map-search-input'
  ) as HTMLInputElement | null;
  mapSearchInput?.addEventListener('input', () => {
    state.mapSearch = mapSearchInput.value;
    renderMapsAtlasSection();
  });

  const modSelect = document.getElementById(
    'filter-mod-select'
  ) as HTMLSelectElement | null;
  modSelect?.addEventListener('change', () => {
    state.modFilter = modSelect.value;
    void refreshAllData();
  });

  const statusSelect = document.getElementById(
    'filter-status-select'
  ) as HTMLSelectElement | null;
  statusSelect?.addEventListener('change', () => {
    state.statusFilter = (statusSelect.value as LobbyStatus | '') || '';
    void refreshAllData();
  });

  const onlyOpenCheck = document.getElementById(
    'filter-only-open'
  ) as HTMLInputElement | null;
  onlyOpenCheck?.addEventListener('change', () => {
    state.onlyOpen = onlyOpenCheck.checked;
    void refreshAllData();
  });

  const searchInput = document.getElementById(
    'filter-search-input'
  ) as HTMLInputElement | null;
  searchInput?.addEventListener('input', () => {
    state.searchQuery = searchInput.value;
    void refreshAllData();
  });

  const autoRefreshCheck = document.getElementById(
    'toggle-auto-refresh'
  ) as HTMLInputElement | null;
  autoRefreshCheck?.addEventListener('change', () => {
    state.autoRefresh = autoRefreshCheck.checked;
  });

  const refreshBtn = document.getElementById('btn-refresh-now');
  refreshBtn?.addEventListener('click', () => {
    void refreshAllData();
    showToast(t(state.lang, 'toast_refreshed'));
  });

  const matchModSelect = document.getElementById(
    'match-filter-mod'
  ) as HTMLSelectElement | null;
  matchModSelect?.addEventListener('change', () => {
    state.matchModFilter = matchModSelect.value;
    void refreshAllData();
  });

  const matchSearchInput = document.getElementById(
    'match-search-input'
  ) as HTMLInputElement | null;
  matchSearchInput?.addEventListener('input', () => {
    state.matchSearchQuery = matchSearchInput.value;
    void refreshAllData();
  });

  document.addEventListener('click', async (e) => {
    const target = (e.target as HTMLElement | null)?.closest('button');
    if (!target) return;

    const setLangAttr = target.getAttribute('data-set-lang') as Locale | null;
    if (setLangAttr === 'de' || setLangAttr === 'en') {
      setLocale(setLangAttr);
      return;
    }

    const selectCrate = target.getAttribute('data-select-crate');
    if (selectCrate) {
      state.selectedCrateId = selectCrate;
      renderHeroCrateInspector();
      return;
    }

    const pipelineTab = target.getAttribute(
      'data-pipeline-tab'
    ) as PipelineTab | null;
    if (pipelineTab) {
      state.pipelineTab = pipelineTab;
      renderPipelineSpecSection();
      return;
    }

    const networkTab = target.getAttribute(
      'data-network-tab'
    ) as NetworkTab | null;
    if (networkTab) {
      state.networkTab = networkTab;
      renderNetworkTabSwitch();
      return;
    }

    const inspectorMode = target.getAttribute(
      'data-inspector-mode'
    ) as InspectorMode | null;
    if (inspectorMode) {
      state.inspectorMode = inspectorMode;
      renderInspectorModeSwitch();
      return;
    }

    const copyText = target.getAttribute('data-copy');
    if (copyText) {
      await copyToClipboard(copyText);
      return;
    }

    const countryId = target.getAttribute('data-country-id');
    if (countryId) {
      state.selectedCountryId = countryId;
      renderCountriesSection();
      return;
    }

    const arsenalFac = target.getAttribute('data-arsenal-faction');
    if (arsenalFac) {
      state.arsenalFaction = arsenalFac;
      renderArsenalSection();
      return;
    }

    const arsenalId = target.getAttribute('data-arsenal-id');
    const arsenalKindAttr = target.getAttribute('data-arsenal-kind') as
      | 'units'
      | 'buildings'
      | null;
    if (arsenalId && arsenalKindAttr) {
      const switchedKind = state.arsenalKind !== arsenalKindAttr;
      state.arsenalKind = arsenalKindAttr;
      state.selectedArsenalId = arsenalId;
      if (switchedKind) {
        state.arsenalFaction = 'all';
        state.arsenalRole = 'all';
        state.arsenalSearch = '';
      }
      renderArsenalSection();
      return;
    }

    const mapCat = target.getAttribute('data-map-category');
    if (mapCat) {
      state.mapCategory = mapCat;
      renderMapsAtlasSection();
      return;
    }

    const selectMapId = target.getAttribute('data-select-map');
    if (selectMapId) {
      state.selectedMapId = selectMapId;
      renderMapsAtlasSection();
      return;
    }

    const previewMapId = target.getAttribute('data-preview-map');
    if (previewMapId) {
      state.inspectorMode = 'maps';
      state.mapCategory = 'all';
      state.mapSearch = '';
      state.selectedMapId = previewMapId;
      renderInspectorModeSwitch();
      renderMapsAtlasSection();
      document
        .getElementById('data-inspector')
        ?.scrollIntoView({ behavior: 'smooth' });
      return;
    }

    const filterMod = target.getAttribute('data-filter-mod');
    if (filterMod !== null) {
      state.modFilter = filterMod;
      state.networkTab = 'lobbies';
      renderNetworkTabSwitch();
      if (modSelect) modSelect.value = filterMod;
      document
        .getElementById('multiplayer-registry')
        ?.scrollIntoView({ behavior: 'smooth' });
      await refreshAllData();
      showToast(`${t(state.lang, 'toast_mod_filter')}: "${filterMod}"`);
      return;
    }
  });
}

document.addEventListener('DOMContentLoaded', () => {
  applyStaticDomTranslations(state.lang);
  renderHeroCrateInspector();
  renderPipelineSpecSection();
  renderNetworkTabSwitch();
  renderInspectorModeSwitch();
  bindEvents();
  void initGamePortalData();
  void refreshAllData();

  window.setInterval(() => {
    if (state.autoRefresh) {
      void refreshAllData();
    }
  }, 3000);

  window.setInterval(() => {
    if (state.turntableDragging || state.inspectorMode !== 'entities') return;
    const canvas = document.getElementById(
      'inspector-3d-canvas'
    ) as HTMLCanvasElement | null;
    const entity = getSelectedArsenalEntity();
    if (canvas && entity) {
      state.turntableYaw += 0.012;
      state.turntableTurret = Math.sin(state.turntableYaw * 1.7) * 0.45;
      render3DTurntableCanvas(
        canvas,
        entity,
        state.turntableYaw,
        state.turntableTurret
      );
    }
  }, 33);
});
