use crate::input::{
    HotkeySlot, MenuScreen, ProductionSidebarTab, UiInputBlocker, VehicleDomainFilter,
};
use rts_simulation::ai::AiDifficulty;

mod actions;
mod actions_campaign;
mod actions_hero_era;
mod actions_match;
mod actions_net;
mod actions_net_tasks;
mod actions_phase21;
mod actions_saves;
mod actions_staging;
mod actions_tactical;
mod actions_workshop;
pub mod control_groups_bar;
pub mod cursors;
pub mod cutscene_director;
pub mod global_production_bar;
pub mod hero_inventory_bar;
pub mod hud_layouts;
mod hud_setup;
mod hud_update;
mod inspector;
pub mod lag_shield_modal;
pub mod locale;
mod minimap;
mod minimap_render;
pub mod objectives_hud;
pub mod player_windows;
pub mod replay_hud;
pub mod screens;
mod sidebar;
mod sidebar_cameos;
mod sidebar_drawers;
pub mod studio_layout;
mod tactical_command_bar;
pub mod tutorial_coach;
pub mod widgets;

pub use actions::handle_sidebar_buttons;
pub use actions_net::{poll_async_lobby_tasks, AsyncLobbyTasks};
pub use control_groups_bar::update_control_groups_bar;
pub use cursors::{update_rts_cursor, RtsCursorKind, RtsCursorState};
pub use cutscene_director::{update_cutscene_and_transmissions, CutsceneDirectorState};
pub use global_production_bar::update_global_production_bar;
pub use hero_inventory_bar::update_hero_inventory_bar;
pub use hud_layouts::sync_hud_layout_preset_and_computed_rects;
pub use hud_setup::setup_rts_hud;
pub use hud_update::{update_cameo_tooltip_ui, update_rts_hud};
pub use lag_shield_modal::update_lag_shield_modal;
pub use minimap::{is_radar_provider_building, update_and_interact_radar_minimap};
pub use objectives_hud::update_objectives_hud;
pub use player_windows::{sync_player_subwindows, PlayerSubWindowKind, PlayerSubWindowStack};
pub use replay_hud::{setup_replay_observer_hud, update_replay_observer_hud};
pub use studio_layout::{
    apply_utf8_font_and_ui_scale, spawn_icon_styled_button, spawn_styled_button,
    update_button_hover_styles, update_selection_drag_box_ui, UiFontResource,
};
pub use tutorial_coach::{update_tutorial_coach, TutorialCoachState};
pub use widgets::update_ui_widgets;

use bevy::prelude::*;
use std::path::PathBuf;

/// Unified enum component replacing the legacy 10-way `Without<...>` text marker queries.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudTextRole {
    TopBarStatus,
    SidebarQueue,
    BottomInspector,
    StudioStatus,
    BattleFeed,
    StudioHeaderTitle,
    SidebarCredits,
    PlayerBottomStance,
    CameoTooltip,
    DevConsoleBody,
}

/// Unified enum component for conflict-free HUD layout & visibility queries.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudLayoutRole {
    TopBarRoot,
    RightSidebarRoot,
    RadarPanelRoot,
    ClassicRadarSlotSpacer,
    GlobalProductionStripRoot,
    HeroInventoryBarRoot,
    BottomCommandBarRoot,
    PlayerBottomStancePill,
    SidebarTelemetryStrip,
    ProductionProgressFill,
    BattleFeedRoot,
    CameoTooltipBanner,
    StudioTopDeck,
    DevConsoleOverlay,
    SmokeTestEguiOverlay,
}

/// Marker placed on every active in-game HUD panel root so `sync_hud_layout_preset_and_computed_rects`
/// blocks 3D world clicks over real Bevy `ComputedNode` bounds across all 3 `HudLayoutPreset`s.
#[derive(Component)]
pub struct HudPanelBlocker;

#[derive(Component)]
pub struct SmokeTestEguiTarget;

#[derive(Resource, Clone)]
pub struct RadarMinimapImage(pub Handle<Image>);

#[derive(Component)]
pub struct RadarMinimapNode;

#[derive(Component)]
pub struct RadarPipVideoNode;

#[derive(Component)]
pub struct DynamicButtonsContainer;

#[derive(Component)]
pub struct StudioModalRoot;

#[derive(Component)]
pub struct StudioModListContainer;

#[derive(Component)]
pub struct SelectionDragBoxNode;

#[derive(Component)]
pub struct PlayerAbilityButtonsContainer;

#[derive(Component, Clone)]
pub struct CameoTooltip(pub String);

#[derive(Component)]
pub struct SidebarCreditsBgNode;

#[derive(Component)]
pub struct SidebarTopBgNode;

#[derive(Component)]
pub struct SidebarOptBtnNode;

#[derive(Component)]
pub struct SidebarDiploBtnNode;

#[derive(Component)]
pub struct SidebarRadarFrameBgNode;

// Marker shims used by `studio_layout.rs` spawn helpers (mapped directly onto `HudTextRole` / `HudLayoutRole`)
#[allow(non_upper_case_globals)]
pub const StudioTopDeckNode: HudLayoutRole = HudLayoutRole::StudioTopDeck;
#[allow(non_upper_case_globals)]
pub const StudioHeaderTitleText: HudTextRole = HudTextRole::StudioHeaderTitle;
#[allow(non_upper_case_globals)]
pub const StudioStatusText: HudTextRole = HudTextRole::StudioStatus;

#[derive(Component, Clone)]
pub struct ButtonVisualStyle {
    pub normal: Color,
    pub hovered: Color,
    pub pressed: Color,
    pub default_border: Color,
}

#[derive(Component, Clone, Debug)]
pub enum SidebarAction {
    StartPlaceBuilding(String),
    OrderWorkerBuild(String),
    ProduceUnit(String),
    ResearchUpgrade(String),
    ReinforceSelectedSquads,
    TriggerAbility,
    TriggerAbilityIndex(usize),
    ToggleAttackMove,
    CycleSelectedStance,
    TogglePatrolMode,
    ToggleGuardMode,
    StopSelectedUnits,
    ScatterSelectedUnits,
    CenterCameraOnSelected,
    CenterCameraOnBase,
    SelectControlGroup(usize),
    ToggleSkirmishAi,
    CycleAiDifficulty,
    SetAiDifficulty(AiDifficulty),
    ToggleFogOfWar,
    CycleGameSpeed,
    TogglePause,
    RepairSelectedBuildings,
    SellSelectedBuildings,
    ToggleRepairCursor,
    ToggleSellCursor,
    UpgradeSelectedBuilding,
    SetPrimaryBuilding,
    UnloadSelectedCarriers,
    CycleAiPersonality,
    CycleEnvironmentPreset,
    QuickSaveMatch,
    QuickLoadMatch,
    SwitchActiveMap(String),
    RestartMatch,
    LoadModFromPath(PathBuf),
    HostLocalMultiplayer,
    JoinLocalMultiplayer,
    JoinSpectatorMatch,
    TriggerHotReconnect,
    CycleHostPlayerSlots,
    BroadcastWorldSync,
    LoadSaveFile(PathBuf),
    PlayReplayFile(PathBuf),
    PlaceLandmine,
    StartCampaignMission {
        campaign_id: String,
        mission_id: String,
    },
    AdvanceNextCampaignMission,
    ToggleAllianceWithPlayer(u8),
    TransferCreditsToPlayer(u8, i32),
    SendQuickTaunt(usize),
    SeekReplayRelative(i64),
    CycleReplayObserverPov,
    FilterSelectionToUnitType(String),
    TriggerSupportPower(String),
    RefreshOnlineLobbies,
    JoinOnlineLobby(String),
    NavigateMenu(MenuScreen),
    EnterLoadingScreen(String),
    FinishLoadingEnterGame,
    ReturnToMainMenu,
    SelectSkirmishCountryIndex(usize),
    CycleSkirmishCountry,
    SelectEncyclopediaFaction(String),
    InspectEncyclopediaEntry(String),
    InspectTechNode(String),
    ToggleLobbyServerEndpoint,
    HotReloadMod,
    SelectCampaignById(String),
    ToggleSkirmishMapFilter,
    GenerateProceduralSkirmishMap,
    PlayModVideo(String),
    VideoTogglePause,
    VideoStopSkip,
    PlayModMusicTrack(String),
    StopModMusic,
    OpenInGamePauseMenu,
    OpenCreatorWorkbench,
    OpenPlayerWindow(PlayerSubWindowKind),
    TogglePlayerWindow(PlayerSubWindowKind),
    ClosePlayerWindow(PlayerSubWindowKind),
    OpenPlayerWindowFromMenu(PlayerSubWindowKind),
    SetOptionsTab(u8),
    SetUiScalePercent(u32),
    SelectProductionTab(ProductionSidebarTab),
    CycleSidebarPage(i32),
    SetVehicleDomainFilter(VehicleDomainFilter),
    CycleSupportPowerPage(i32),
    CycleUpgradePage(i32),
    ToggleEncyclopediaCategory,
    ToggleAudioMute,
    ToggleEdgeScroll,
    StartRebindHotkey(HotkeySlot),
    ResetHotkeysToDefaults,
    ToggleGridHotkeys,
    ToggleWasdPan,
    SetSsaoQuality(String),
    SetLanguage(String),
    CycleAutosaveInterval,
    ToggleHoldProduction(String),
    SaveNamedSlot,
    LoadSaveSlot(PathBuf),
    DeleteSaveSlot(PathBuf),
    StagingCycleSlotOccupant(u8),
    StagingCycleSlotFaction(u8),
    StagingCycleSlotSubFaction(u8),
    StagingCycleSlotColor(u8),
    StagingCycleSlotTeam(u8),
    StagingSelectSpawn(u8),
    StagingCycleSlotHandicap(u8),
    StagingToggleReady,
    StagingLaunchMatch,
    StagingSendChat(bool),
    StagingCycleRuleMap,
    StagingCycleRuleCredits,
    StagingCycleRuleSpeed,
    StagingToggleSuperweapons,
    StagingToggleCrates,
    StagingToggleFogOfWar,
    StagingCycleGameMode,
    StagingPublishCustomMap,
    LeaveStagingRoom,
    StartRankedAutomatch1v1,
    StartRankedAutomatch2v2,
    CancelRankedAutomatch,
    RefreshLeaderboard,
    SetPostGameTab(u8),
    ContinueViewingBattlefield,
    OpenPostGameScoreScreen,
    SaveMatchReplayNow,
    SetReplaySpeedTenths(u32),
    SeekReplayAbsoluteTick(u64),
    SetReplayObserverPov(u8),
    TogglePingCursor,
    CycleIdleWorker,
    JukeboxPlayTrack(usize),
    JukeboxTogglePause,
    JukeboxNextTrack,
    JukeboxPrevTrack,
    JukeboxToggleShuffle,
    JukeboxToggleRepeat,
    JukeboxToggleDynamicCombat,
    JukeboxCycleFactionFilter,
    JukeboxCyclePage(i32),
    CycleWindowMode,
    CycleResolution(bool),
    ToggleVsync,
    SetShadowQuality(String),
    CycleFpsCap,
    ToggleObjectivesHudCollapse,
    JumpCameraToTile([i32; 2]),
    SkipActiveCutscene,
    LaunchBootcampTutorial,
    DismissTutorialCoach,
    AdvanceTutorialStep,
    UnlockCommanderNode(String),
    RetreatSelectedSquads,
    ToggleSelectedSquadAutoReinforce,
    UpgradeSelectedSquadWeapon(String),
    ToggleSelectedBurrow,
    UpgradeSelectedBuildingTo(String),
    CastLagStallVoteDrop(u8),
    CastLagStallVoteAi(u8),
    StagingCycleResumeSavePage(i32),
    StagingSelectResumeSave(PathBuf),
    StagingClearResumeSave,
    StagingToggleAllowSpectators,
    StagingCycleSpectatorDelay,
    SpectateOnlineLobby(String),
    RefreshWorkshopItems,
    SetWorkshopKindFilter(Option<rts_network::lobby_registry::WorkshopItemKind>),
    CycleWorkshopSort,
    CycleWorkshopPage(i32),
    DownloadWorkshopItem {
        item_id: String,
        sha256: Option<String>,
        auto_join_addr: Option<String>,
    },
    DismissMissingModPrompt,
    ToggleHarvestCursor,
    AdvancePlayerEra(String),
    CancelLocalResearch {
        building_id: u64,
        upgrade_id: String,
    },
    LearnHeroSkill {
        hero_id: u64,
        ability_id: String,
    },
    ReviveFallenHero(String),
    UseHeroInventorySlot {
        hero_id: u64,
        slot_index: usize,
    },
    CycleHudLayoutPreset,
    CycleColorblindMode,
    ToggleSmartCast,
    ToggleAutoResyncOnDesync,
    ToggleMutatorOverlay(String),
    StagingToggleMutator(String),
    StagingToggleHybridRollback,
    RejoinSavedMatch,
    DismissSavedReconnect,
    ApplyAuthoritativeResync,
    ExitApplication,
}

#[derive(Resource, Clone)]
pub struct PostGameGraphImage(pub Handle<Image>);

#[derive(Resource, Default)]
pub struct LastSidebarSignature {
    pub sidebar_sig: String,
    pub studio_sig: String,
    pub ability_sig: String,
}
