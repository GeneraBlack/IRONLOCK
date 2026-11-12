export type LobbyStatus = 'open' | 'full' | 'in_game';

export interface LobbyAnnouncement {
  lobby_name: string;
  host_player: string;
  host_addr: string;
  mod_id: string;
  mod_name: string;
  mod_version: string;
  map_id: string;
  map_name: string;
  current_players: number;
  max_players: number;
  game_mode: string;
  password_protected?: boolean;
  password?: string | null;
  allocate_relay?: boolean;
  prefer_mux_relay?: boolean;
  upnp_external_addr?: string | null;
  host_token?: string | null;
  hostToken?: string | null;
}

export interface LobbyEntry {
  lobby_id: string;
  lobby_name: string;
  host_player: string;
  host_addr: string;
  external_addr?: string | null;
  relay_addr?: string | null;
  relay_uplink_addr?: string | null;
  relay_room_token?: string | null;
  relay_mux_port?: number | null;
  udp_rendezvous_port?: number | null;
  udp_rendezvous_addr?: string | null;
  nat_mode: string;
  mod_id: string;
  mod_name: string;
  mod_version: string;
  map_id: string;
  map_name: string;
  current_players: number;
  max_players: number;
  status: LobbyStatus;
  game_mode: string;
  password_protected: boolean;
  created_at_unix_ms: number;
  last_heartbeat_unix_ms: number;
}

export interface RegisteredLobbyResponse extends LobbyEntry {
  hostToken: string;
  host_token: string;
}

export interface LobbyHeartbeatRequest {
  current_players: number;
  max_players: number;
  status?: LobbyStatus | null;
  map_id?: string | null;
  map_name?: string | null;
  host_token?: string | null;
  hostToken?: string | null;
}

export interface LobbyListResponse {
  lobbies: LobbyEntry[];
}

export interface MatchPlayerRecord {
  name: string;
  faction: string;
  team?: number;
  is_winner: boolean;
  player_id?: string | null;
}

export interface MatchRecord {
  match_id: string;
  lobby_id?: string | null;
  lobby_name: string;
  mod_id: string;
  mod_name: string;
  mod_version: string;
  map_id: string;
  map_name: string;
  game_mode: string;
  players: MatchPlayerRecord[];
  winner_player: string;
  winner_faction: string;
  duration_secs: number;
  final_tick: number;
  final_state_hash: string;
  total_commands: number;
  desync_detected: boolean;
  replay_file?: string | null;
  replay_url?: string | null;
  replayUrl?: string | null;
  completed_at_unix_ms: number;
}

export interface CreateMatchRequest {
  lobby_id?: string | null;
  lobby_name?: string;
  mod_id?: string;
  mod_name?: string;
  mod_version?: string;
  map_id?: string;
  map_name?: string;
  game_mode?: string;
  players?: MatchPlayerRecord[];
  winner_player?: string;
  winner_faction?: string;
  duration_secs?: number;
  final_tick?: number;
  final_state_hash?: string | number;
  total_commands?: number;
  desync_detected?: boolean;
  replay_file?: string;
  replay_url?: string | null;
  replayUrl?: string | null;
}

export interface MatchListResponse {
  matches: MatchRecord[];
}

export interface CommanderRankEntry {
  rank: number;
  player_name: string;
  player?: string;
  main_faction: string;
  favorite_faction?: string;
  wins: number;
  losses: number;
  matches_played: number;
  games?: number;
  win_rate_pct: number;
  total_ticks: number;
  verified_matches: number;
  last_played_unix_ms: number;
  rating: number;
  rd: number;
  streak: number;
  player_id?: string | null;
}

export interface LeaderboardResponse {
  leaderboard: CommanderRankEntry[];
}

export interface StunWhoamiResponse {
  ip: string;
  port: number;
  is_cgnat: boolean;
}

export interface PlayerRegistrationRequest {
  player_id: string;
  display_name: string;
  public_token: string;
}

export interface SignedMatchReport extends CreateMatchRequest {
  player_id: string;
  submitter_name: string;
  public_token: string;
  timestamp_unix_ms: number;
  signature: string;
  match_request?: CreateMatchRequest;
}

export type AutomatchMode = '1v1' | '2v2';
export type AutomatchPollStatus =
  | 'searching'
  | 'matched'
  | 'cancelled'
  | 'expired';

export interface AutomatchEnqueueRequest {
  player_id: string;
  display_name: string;
  public_token: string;
  mod_id?: string;
  mode: AutomatchMode;
  preferred_faction?: string;
  host_addr?: string | null;
  upnp_external_addr?: string | null;
}

export interface AutomatchPlayerAssignment {
  player_id: string;
  display_name: string;
  slot_index: number;
  assigned_player_id: number;
  team_id: number;
  preferred_faction: string;
  rating: number;
}

export interface AutomatchAssignment {
  match_token: string;
  lobby_id: string;
  mode: AutomatchMode;
  mod_id: string;
  map_id: string;
  random_seed: number;
  host_player_id: string;
  is_host: boolean;
  assigned_player_id: number;
  relay_addr: string;
  relay_uplink_addr: string;
  relay_room_token: string;
  candidate_addrs: string[];
  players: AutomatchPlayerAssignment[];
  authoritative_dedicated?: boolean;
  dedicated_server_id?: string | null;
  dedicated_addr?: string | null;
}

export type DedicatedServerStatus =
  | 'idle'
  | 'allocated'
  | 'staging'
  | 'in_game'
  | 'draining';

export interface DedicatedMatchAllocation {
  match_token: string;
  lobby_id: string;
  mode: string;
  mod_id: string;
  active_mutators: string[];
  map_id: string;
  random_seed: number;
  expected_players: number;
  players: AutomatchPlayerAssignment[];
  allocated_at_unix_ms: number;
}

export interface DedicatedServerRegisterRequest {
  server_id?: string;
  display_name?: string;
  tcp_addr: string;
  udp_port?: number | null;
  region?: string;
  mod_id: string;
  mod_version?: string;
  mod_content_hash?: string;
  active_mutators?: string[];
  max_players?: number;
  current_players?: number;
  status?: DedicatedServerStatus;
  allocate_relay?: boolean;
  fleet_secret?: string | null;
  server_token?: string | null;
}

export interface DedicatedServerEntry {
  server_id: string;
  display_name: string;
  tcp_addr: string;
  udp_port: number | null;
  region: string;
  mod_id: string;
  mod_version: string;
  mod_content_hash: string;
  active_mutators: string[];
  max_players: number;
  current_players: number;
  active_tick: number;
  status: DedicatedServerStatus;
  allocated_match_token: string | null;
  allocated_lobby_id: string | null;
  allocation: DedicatedMatchAllocation | null;
  relay_addr: string | null;
  relay_uplink_addr: string | null;
  relay_room_token: string | null;
  registered_at_unix_ms: number;
  last_heartbeat_unix_ms: number;
  matches_hosted: number;
}

export interface DedicatedServerRegisterResponse extends DedicatedServerEntry {
  server_token: string;
}

export interface DedicatedServerHeartbeatRequest {
  server_id: string;
  server_token?: string | null;
  status?: DedicatedServerStatus;
  current_players?: number;
  max_players?: number;
  active_tick?: number;
  mod_id?: string;
  active_mutators?: string[];
  completed_match_token?: string | null;
}

export interface DedicatedServerAllocateRequest {
  mod_id: string;
  mod_version?: string;
  active_mutators?: string[];
  required_players?: number;
  region?: string;
  map_id?: string;
  mode?: string;
  match_token?: string;
  lobby_id?: string;
  random_seed?: number;
  players?: AutomatchPlayerAssignment[];
}

export interface DedicatedServerListResponse {
  servers: DedicatedServerEntry[];
  total: number;
  idle: number;
  active: number;
}

export interface AutomatchPollResponse {
  ticket_id: string;
  status: AutomatchPollStatus;
  rating: number;
  search_window: number;
  assignment?: AutomatchAssignment;
}

export interface AutomatchCancelRequest {
  ticket_id: string;
  player_id: string;
}

export interface RegistryStats {
  status: 'online';
  server_version: string;
  public_domain: string;
  node_id: string;
  uptime_secs: number;
  heartbeat_ttl_secs: number;
  active_lobbies: number;
  open_lobbies: number;
  full_lobbies: number;
  in_game_lobbies: number;
  players_online: number;
  total_lobbies_registered: number;
  total_matches_completed: number;
  relay_mux_port?: number | null;
  relay_rooms_active?: number;
  udp_rendezvous_port?: number | null;
  workshop_items_total?: number;
  dedicated_servers_online?: number;
  dedicated_servers_idle?: number;
  mods_active: Record<string, { lobbies: number; players: number }>;
}

export type WorkshopItemKind = 'mod' | 'map';

export type WorkshopSortOrder =
  | 'downloads'
  | 'newest'
  | 'updated'
  | 'title';

export interface WorkshopItemMetadata {
  id: string;
  title: string;
  description: string;
  author_name: string;
  author_player_id: string;
  kind: WorkshopItemKind;
  version: string;
  tags: string[];
  sha256: string;
  mod_content_hash: string;
  size_bytes: number;
  file_name: string;
  downloads: number;
  created_at_unix_ms: number;
  updated_at_unix_ms: number;
}

export interface WorkshopItemListResponse {
  items: WorkshopItemMetadata[];
  total: number;
}

export interface WorkshopPublishMetadataInput {
  id: string;
  title: string;
  description?: string;
  author_name: string;
  player_id: string;
  public_token?: string;
  timestamp_unix_ms?: number;
  signature?: string;
  kind: WorkshopItemKind;
  version: string;
  tags?: string[];
  sha256: string;
  mod_content_hash: string;
}

export interface WorkshopPublishJsonRequest extends WorkshopPublishMetadataInput {
  payload_base64: string;
}

export interface MapSummary {
  id: string;
  name: string;
}

export interface ModSummary {
  id: string;
  name: string;
  version: string;
  description: string;
  entry_map: string;
  scripts: string[];
  factions_count: number;
  units_count: number;
  buildings_count: number;
  weapons_count: number;
  maps: MapSummary[];
}

export interface EngineCrateInfo {
  name: string;
  role: string;
  highlights: string[];
}

export interface ProjectOverview {
  engine_name: string;
  engine_version: string;
  tick_rate_hz: number;
  crates: EngineCrateInfo[];
  mods: ModSummary[];
}

export interface WeaponDetails {
  id: string;
  name: string;
  damage: number;
  cooldown_secs: number;
  burst: number;
  dps: number;
  range: number;
  projectile_speed: number;
  projectile_style: string;
  splash_radius: number;
  targets_ground: boolean;
  targets_air: boolean;
  armor_multipliers: Record<string, number>;
}

export interface EntityVisualSpec {
  archetype: string;
  scale: number[];
  primary_color: number[];
  model: string;
}

export interface UnitAbilitySpec {
  id: string;
  name: string;
  cooldown_secs: number;
  target_mode: string;
}

export interface SupportPowerSpec {
  id: string;
  name: string;
  kind: string;
  cooldown_secs: number;
  cost: number;
  radius: number;
  duration_secs: number;
}

export interface UnitRecord {
  id: string;
  name: string;
  faction: string;
  role: string;
  movement_domain: string;
  cost: number;
  supply_cost: number;
  build_time_secs: number;
  health: number;
  armor: string;
  speed: number;
  vision_range: number;
  primary_weapon?: string | null;
  weapon_details?: WeaponDetails | null;
  prerequisites: string[];
  ability_id?: string | null;
  abilities: UnitAbilitySpec[];
  crushes_infantry: boolean;
  detects_cloak: boolean;
  cloaked: boolean;
  passenger_slots: number;
  harvest_capacity: number;
  cameo: string;
  visual: EntityVisualSpec;
  mesh_vertices?: number;
  mesh_triangles?: number;
  mesh_bytes?: number;
}

export interface BuildingRecord {
  id: string;
  name: string;
  faction: string;
  role: string;
  cost: number;
  build_time_secs: number;
  health: number;
  armor: string;
  footprint: number[];
  power_delta: number;
  supply_provided: number;
  vision_range: number;
  garrison_capacity: number;
  primary_weapon?: string | null;
  weapon_details?: WeaponDetails | null;
  prerequisites: string[];
  upgrades_to?: string | null;
  spawns_unit_on_complete?: string | null;
  support_powers: SupportPowerSpec[];
  cameo: string;
  visual: EntityVisualSpec;
  mesh_vertices?: number;
  mesh_triangles?: number;
  mesh_bytes?: number;
}

export interface CountryDescriptor {
  id: string;
  name: string;
  faction: string;
  special_unit: string;
  special_cameo: string;
  doctrine: string;
  artwork: string;
}

export interface MapRadarSpawn {
  player_id: number;
  faction: string;
  base_tile: number[];
  initial_buildings_count: number;
  initial_units_count: number;
}

export interface MapRadarOrePatch {
  position: number[];
  radius: number;
  amount_per_tile: number;
}

export interface MapRadarRect {
  min: number[];
  max: number[];
}

export interface MapTechStructure {
  building_id: string;
  tile_pos: number[];
}

export interface MapRadarRecord {
  id: string;
  name: string;
  category: string;
  width: number;
  height: number;
  tile_size: number;
  ore_regen_per_sec: number;
  mission_objective: string;
  max_players: number;
  theater?: string;
  preview_webp?: string;
  structures_count?: number;
  trees_count?: number;
  units_count?: number;
  elevation_min?: number;
  elevation_max?: number;
  preplaced_structures?: Array<{ id: string; faction: string; nx: number; ny: number }>;
  spawns: MapRadarSpawn[];
  ore_patches: MapRadarOrePatch[];
  obstacles: MapRadarRect[];
  elevation_rects: MapRadarRect[];
  ramp_rects: MapRadarRect[];
  water_rects: MapRadarRect[];
  bridge_rects: MapRadarRect[];
  tech_structures: MapTechStructure[];
}

export interface FactionInfo {
  id: string;
  name: string;
  color: number[];
  starting_credits: number;
  description: string;
}

export interface GameDatabase {
  game_title: string;
  engine_version: string;
  tick_rate_hz: number;
  factions: FactionInfo[];
  countries: CountryDescriptor[];
  units: UnitRecord[];
  buildings: BuildingRecord[];
  weapons: WeaponDetails[];
  maps: MapRadarRecord[];
}
