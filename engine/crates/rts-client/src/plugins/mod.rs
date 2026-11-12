pub mod lobby_reporting;
pub mod match_lifecycle;
pub mod net_runtime;
pub mod state_events;

use crate::audio::RtsAudioPlugin;
use crate::camera::{update_rts_camera, RtsCameraPlugin};
use crate::input::{
    handle_rts_input, AppPhase, ClientInteractionState, MenuScreen, UiAssetCache, UiInputBlocker,
};
use crate::rendering::{
    draw_rts_gizmos, refresh_cached_fog_visibility, setup_3d_map_environment,
    sync_sim_entities_to_3d, ClientRenderingAvPlugin,
};
use crate::smoke_harness::drive_visual_smoke_test;
use crate::studio::CreatorWorkbenchPlugin;
use crate::ui::{
    apply_utf8_font_and_ui_scale, handle_sidebar_buttons, poll_async_lobby_tasks,
    setup_replay_observer_hud, setup_rts_hud, sync_hud_layout_preset_and_computed_rects,
    sync_player_subwindows, update_and_interact_radar_minimap, update_button_hover_styles,
    update_cameo_tooltip_ui, update_control_groups_bar, update_cutscene_and_transmissions,
    update_global_production_bar, update_hero_inventory_bar, update_lag_shield_modal,
    update_objectives_hud, update_replay_observer_hud, update_rts_cursor, update_rts_hud,
    update_selection_drag_box_ui, update_tutorial_coach, update_ui_widgets, AsyncLobbyTasks,
    CutsceneDirectorState, LastSidebarSignature, PlayerSubWindowStack, RtsCursorState,
    TutorialCoachState, UiFontResource,
};
use crate::vfx::{setup_vfx_assets, spawn_combat_and_destruction_vfx, update_and_draw_vfx};
use bevy::app::{PluginGroup, PluginGroupBuilder};
use bevy::prelude::*;
use bevy::state::app::AppExtStates;

pub use match_lifecycle::{
    drive_autosave_and_postgame_transitions, normalize_asset_root_path,
    resolve_bundle_launch_config, resolve_workspace_root_for_mod, sync_pending_environment_rebuild,
    AssetRootResource, ClientCli, ModResource, SimWorldResource, VisualSmokeTestState,
};
pub use net_runtime::{
    ensure_local_lobby_webserver_running, resolve_and_ensure_lobby_webserver,
    step_fixed_simulation, NetRuntimeResource,
};
pub use state_events::{
    apply_live_window_and_display_settings, register_mod_vfs_asset_source,
    sync_states_and_domain_resources, AppliedDisplaySettingsCache, CommandCursorDomainState,
    DesyncRecoveryEvent, DisplayAudioDomainState, HudPresetChangedEvent, LayeredModVfsAssetReader,
    MatchLifecycleEvent, ModSwitchEvent, MultiplayerSessionDomainState, ReplayControlEvent,
    RtsSystemSet, SelectionDomainState,
};

/// Core ECS `States<AppPhase>` / `States<MenuScreen>`, typed `Event`s, domain sub-resources,
/// and deterministic `RtsSystemSet` frame ordering (`Task 235`).
#[derive(Default)]
pub struct RtsStatePlugin;

impl Plugin for RtsStatePlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<ClientInteractionState>() {
            app.insert_resource(ClientInteractionState::new(
                1,
                false,
                false,
                "OFFLINE SKIRMISH".to_string(),
            ));
        }
        if !app.world().contains_resource::<VisualSmokeTestState>() {
            app.init_resource::<VisualSmokeTestState>();
        }
        app.init_resource::<AppliedDisplaySettingsCache>()
            .init_resource::<SelectionDomainState>()
            .init_resource::<CommandCursorDomainState>()
            .init_resource::<MultiplayerSessionDomainState>()
            .init_resource::<DisplayAudioDomainState>()
            .add_event::<MatchLifecycleEvent>()
            .add_event::<ModSwitchEvent>()
            .add_event::<ReplayControlEvent>()
            .add_event::<HudPresetChangedEvent>()
            .add_event::<DesyncRecoveryEvent>()
            .configure_sets(
                Update,
                (
                    RtsSystemSet::EnvRebuild,
                    RtsSystemSet::FogRefresh,
                    RtsSystemSet::SubwindowsAndWidgets,
                    RtsSystemSet::CameraAndInput,
                    RtsSystemSet::SimVisualSync,
                    RtsSystemSet::HudUpdate,
                )
                    .chain(),
            );

        if app.is_plugin_added::<bevy::state::app::StatesPlugin>() {
            app.init_state::<AppPhase>().init_state::<MenuScreen>();
        }

        app.add_systems(Startup, (setup_3d_map_environment, setup_vfx_assets))
            .add_systems(
                Update,
                (
                    sync_pending_environment_rebuild
                        .in_set(RtsSystemSet::EnvRebuild)
                        .before(refresh_cached_fog_visibility),
                    refresh_cached_fog_visibility.in_set(RtsSystemSet::FogRefresh),
                    sync_states_and_domain_resources.in_set(RtsSystemSet::FogRefresh),
                    apply_live_window_and_display_settings
                        .in_set(RtsSystemSet::SubwindowsAndWidgets)
                        .before(apply_utf8_font_and_ui_scale),
                    (
                        sync_sim_entities_to_3d,
                        spawn_combat_and_destruction_vfx,
                        update_and_draw_vfx,
                        draw_rts_gizmos,
                        drive_autosave_and_postgame_transitions,
                    )
                        .in_set(RtsSystemSet::SimVisualSync)
                        .after(refresh_cached_fog_visibility),
                ),
            );
    }
}

/// Tactical keyboard, mouse, Smart-Cast, Circle-Drag Area Commands, and Curved Formation input plugin (`Tasks 235 & 239`).
#[derive(Default)]
pub struct RtsInputPlugin;

impl Plugin for RtsInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiAssetCache>()
            .init_resource::<UiInputBlocker>()
            .add_systems(
                Update,
                handle_rts_input
                    .in_set(RtsSystemSet::CameraAndInput)
                    .after(refresh_cached_fog_visibility)
                    .after(update_rts_camera),
            );
    }
}

/// Data-driven `HudLayoutPreset` HUD, Global Production/Era/Streaming Strip, Hero 6-Slot Inventory,
/// Minimap, and Player Sub-Windows plugin (`Tasks 235 & 240`).
#[derive(Default)]
pub struct RtsHudPlugin;

impl Plugin for RtsHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CutsceneDirectorState>()
            .init_resource::<TutorialCoachState>()
            .init_resource::<PlayerSubWindowStack>()
            .init_resource::<RtsCursorState>()
            .init_resource::<UiFontResource>()
            .init_resource::<AsyncLobbyTasks>()
            .init_resource::<LastSidebarSignature>()
            .add_systems(Startup, (setup_rts_hud, setup_replay_observer_hud))
            .add_systems(
                Update,
                (
                    apply_utf8_font_and_ui_scale.in_set(RtsSystemSet::SubwindowsAndWidgets),
                    sync_player_subwindows
                        .in_set(RtsSystemSet::SubwindowsAndWidgets)
                        .before(update_rts_camera)
                        .before(handle_rts_input),
                    update_ui_widgets
                        .in_set(RtsSystemSet::SubwindowsAndWidgets)
                        .before(update_rts_camera)
                        .before(handle_rts_input),
                    update_cutscene_and_transmissions
                        .in_set(RtsSystemSet::SubwindowsAndWidgets)
                        .before(update_rts_camera)
                        .before(handle_rts_input),
                    sync_hud_layout_preset_and_computed_rects
                        .in_set(RtsSystemSet::SubwindowsAndWidgets)
                        .before(handle_rts_input)
                        .before(handle_sidebar_buttons),
                    handle_sidebar_buttons
                        .in_set(RtsSystemSet::CameraAndInput)
                        .after(handle_rts_input),
                    update_and_interact_radar_minimap
                        .in_set(RtsSystemSet::CameraAndInput)
                        .after(handle_sidebar_buttons),
                    update_button_hover_styles
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(handle_sidebar_buttons),
                ),
            )
            .add_systems(
                Update,
                (
                    update_selection_drag_box_ui.in_set(RtsSystemSet::HudUpdate),
                    update_rts_hud.in_set(RtsSystemSet::HudUpdate),
                    update_global_production_bar
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(update_rts_hud),
                    update_hero_inventory_bar
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(update_rts_hud),
                    update_control_groups_bar
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(update_rts_hud),
                    update_objectives_hud
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(update_rts_hud),
                    update_tutorial_coach
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(update_rts_hud),
                    update_lag_shield_modal
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(update_rts_hud),
                    update_replay_observer_hud
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(handle_sidebar_buttons),
                    update_cameo_tooltip_ui
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(update_hero_inventory_bar),
                    update_rts_cursor
                        .in_set(RtsSystemSet::HudUpdate)
                        .after(handle_rts_input),
                ),
            );
    }
}

/// Fixed-step lockstep / 1-tick hybrid rollback, crash-reconnect, and lobby task polling plugin (`Tasks 235, 248, 250`).
#[derive(Default)]
pub struct RtsNetLockstepPlugin;

impl Plugin for RtsNetLockstepPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<NetRuntimeResource>() {
            app.init_resource::<NetRuntimeResource>();
        }
        app.insert_resource(Time::<Fixed>::from_hz(20.0))
            .add_systems(FixedUpdate, step_fixed_simulation)
            .add_systems(
                Update,
                poll_async_lobby_tasks.in_set(RtsSystemSet::SubwindowsAndWidgets),
            );
    }
}

/// Automated 14-stage (`01..14`) Vulkan visual smoke harness plugin (`Tasks 235 & 252`).
#[derive(Default)]
pub struct VisualSmokeHarnessPlugin;

impl Plugin for VisualSmokeHarnessPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<VisualSmokeTestState>() {
            app.init_resource::<VisualSmokeTestState>();
        }
        app.add_systems(PostUpdate, drive_visual_smoke_test);
    }
}

/// Complete, embeddable `PluginGroup` bundling all 9 domain plugins of the modular Rust 3D RTS Engine (`Task 235`).
#[derive(Default)]
pub struct RtsEngineClientPlugins;

impl PluginGroup for RtsEngineClientPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(RtsStatePlugin)
            .add(RtsCameraPlugin)
            .add(ClientRenderingAvPlugin)
            .add(RtsInputPlugin)
            .add(RtsHudPlugin)
            .add(RtsAudioPlugin)
            .add(RtsNetLockstepPlugin)
            .add(CreatorWorkbenchPlugin)
            .add(VisualSmokeHarnessPlugin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;
    use bevy::window::WindowPlugin;

    #[test]
    fn rts_engine_client_plugins_headless_schedule_builds_with_zero_b0001_b0002_conflicts() {
        let mod_dir = rts_sdk::loader::LoadedMod::resolve_mod_dir("rust-ra2");
        let loaded = rts_sdk::loader::LoadedMod::load_from_dir(mod_dir)
            .expect("load rust-ra2 for headless schedule test");
        let sim_world = rts_simulation::world::SimWorld::from_mod(&loaded);
        let workspace_root = resolve_workspace_root_for_mod(&loaded.root_dir);

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            InputPlugin,
            WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            },
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ))
        .init_asset::<Shader>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<bevy::gltf::Gltf>()
        .init_asset::<bevy::gltf::GltfMesh>()
        .init_asset::<bevy::gltf::GltfNode>()
        .init_asset::<bevy::gltf::GltfPrimitive>()
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<bevy::animation::graph::AnimationGraph>()
        .init_asset::<bevy::audio::AudioSource>()
        .add_plugins(bevy::gizmos::GizmoPlugin)
        .insert_resource(AssetRootResource(workspace_root))
        .insert_resource(ModResource(loaded))
        .insert_resource(SimWorldResource(sim_world))
        .add_plugins(
            RtsEngineClientPlugins
                .build()
                .disable::<CreatorWorkbenchPlugin>(),
        );

        // Running two full updates initializes all schedules (`Startup`, `Update`, `PostUpdate`)
        // and panics immediately if ANY `B0001` query conflict or `B0002` resource conflict exists!
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppPhase>>().get(),
            AppPhase::MainMenu
        );
        assert!(app.world().contains_resource::<SelectionDomainState>());
        assert!(app.world().contains_resource::<CommandCursorDomainState>());
        assert!(app
            .world()
            .contains_resource::<MultiplayerSessionDomainState>());
    }
}
