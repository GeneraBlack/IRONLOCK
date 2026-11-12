pub mod briefing;
pub mod commander_tree;
pub mod diplomacy;
pub mod jukebox;
pub mod logbook;
pub mod options;
pub mod options_hotkeys;
pub mod save_load;
pub mod tech_tree;

use crate::audio::{JukeboxState, SoundSettingsResource};
use crate::input::{AppPhase, ClientInteractionState, UiAssetCache, UiInputBlocker};
use crate::rendering::VideoPlaybackResource;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button, UiFontResource};
use crate::ui::widgets::ui_node_logical_rect;
use crate::ui::SidebarAction;
use crate::{AssetRootResource, ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::loader::LoadedMod;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerSubWindowKind {
    Briefing,
    Diplomacy,
    TechTree,
    Logbook,
    Options,
    SaveLoad,
    Jukebox,
    CommanderTree,
}

impl PlayerSubWindowKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Briefing => "MISSION BRIEFING & IN-ENGINE FMV",
            Self::Diplomacy => "DIPLOMACY, TRIBUTE & COMMS [D]",
            Self::TechTree => "FACTION TECHNOLOGY TREE [Y]",
            Self::Logbook => "MISSION OBJECTIVES & EVA LOGBOOK [TAB]",
            Self::Options => "TACTICAL CLIENT OPTIONS",
            Self::SaveLoad => "SAVE & LOAD MATCH SLOTS [F5 / F8]",
            Self::Jukebox => "IN-GAME SOUNDTRACK JUKEBOX [J]",
            Self::CommanderTree => "COMMANDER PROMOTIONS & DOCTRINE TREE",
        }
    }

    pub fn default_position(self) -> (f32, f32, f32) {
        // (left_px, top_px, width_px)
        match self {
            Self::Briefing => (110.0, 56.0, 620.0),
            Self::Diplomacy => (90.0, 66.0, 560.0),
            Self::TechTree => (72.0, 52.0, 760.0),
            Self::Logbook => (120.0, 64.0, 580.0),
            Self::Options => (130.0, 48.0, 630.0),
            Self::SaveLoad => (136.0, 54.0, 600.0),
            Self::Jukebox => (594.0, 48.0, 542.0),
            Self::CommanderTree => (24.0, 48.0, 558.0),
        }
    }
}

/// Manages floating mod-skinned In-Game Player sub-windows with LIFO `[ESC]` priority
/// and pointer hit-testing so clicks over open windows never leak to the 3D battlefield.
#[derive(Resource, Debug, Default, Clone)]
pub struct PlayerSubWindowStack {
    pub stack: Vec<PlayerSubWindowKind>,
    pub window_rects: HashMap<PlayerSubWindowKind, Rect>,
    pub inspected_tech_id: Option<String>,
    pub last_rendered_sig: String,
    pub has_spawned_children: bool,
}

impl PlayerSubWindowStack {
    pub fn is_open(&self, kind: PlayerSubWindowKind) -> bool {
        self.stack.contains(&kind)
    }

    pub fn topmost(&self) -> Option<PlayerSubWindowKind> {
        self.stack.last().copied()
    }

    pub fn open(&mut self, kind: PlayerSubWindowKind) {
        self.stack.retain(|&k| k != kind);
        self.stack.push(kind);
        self.last_rendered_sig.clear();
    }

    pub fn open_or_focus(&mut self, kind: PlayerSubWindowKind) {
        self.open(kind);
    }

    pub fn close(&mut self, kind: PlayerSubWindowKind) {
        self.stack.retain(|&k| k != kind);
        self.window_rects.remove(&kind);
        self.last_rendered_sig.clear();
    }

    pub fn close_all(&mut self) {
        self.clear();
    }

    pub fn toggle(&mut self, kind: PlayerSubWindowKind) {
        if self.topmost() == Some(kind) {
            self.close(kind);
        } else {
            self.open(kind);
        }
    }

    /// Closes the topmost sub-window in LIFO order. Returns `Some(closed)` if a window was closed,
    /// allowing `[ESC]` to close windows one-by-one before opening the Pause Menu.
    pub fn close_topmost(&mut self) -> Option<PlayerSubWindowKind> {
        let popped = self.stack.pop()?;
        self.window_rects.remove(&popped);
        self.last_rendered_sig.clear();
        Some(popped)
    }

    pub fn clear(&mut self) {
        self.stack.clear();
        self.window_rects.clear();
        self.last_rendered_sig.clear();
    }

    pub fn contains_pointer(&self, cursor: Vec2) -> bool {
        self.stack.iter().any(|kind| {
            self.window_rects
                .get(kind)
                .is_some_and(|r| r.contains(cursor))
        })
    }

    pub fn contains_screen_point(&self, cursor: Vec2, _screen_size: Vec2) -> bool {
        self.contains_pointer(cursor)
    }
}

#[derive(Component)]
pub struct PlayerSubWindowRoot;

#[derive(Component)]
pub struct PlayerSubWindowPanel(pub PlayerSubWindowKind);

#[derive(Component)]
pub struct BriefingVideoImageNode;

#[derive(Component)]
pub struct BriefingVideoProgressFill;

#[derive(Component)]
pub struct BriefingVideoSubtitleText;

pub fn resolve_faction_window_palette(
    loaded: &LoadedMod,
    faction_id: &str,
) -> (Color, Color, Color) {
    let rgb = loaded
        .factions
        .get(faction_id)
        .or_else(|| {
            loaded
                .factions
                .values()
                .find(|f| f.id.eq_ignore_ascii_case(faction_id))
        })
        .or_else(|| loaded.factions.values().next())
        .map(|f| f.color)
        .unwrap_or([0.25, 0.55, 0.92]);

    let header_bg = Color::srgba(
        (rgb[0] * 0.42 + 0.05).clamp(0.0, 0.78),
        (rgb[1] * 0.42 + 0.06).clamp(0.0, 0.78),
        (rgb[2] * 0.42 + 0.09).clamp(0.0, 0.78),
        0.97,
    );
    let border_col = Color::srgb(
        (rgb[0] * 0.75 + 0.22).clamp(0.2, 1.0),
        (rgb[1] * 0.75 + 0.22).clamp(0.2, 1.0),
        (rgb[2] * 0.75 + 0.22).clamp(0.2, 1.0),
    );
    let panel_bg = Color::srgba(0.04, 0.06, 0.10, 0.95);
    (panel_bg, header_bg, border_col)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_player_subwindows(
    mut commands: Commands,
    (
        asset_server,
        asset_root,
        mod_res,
        sim_res,
        interaction,
        sound_res,
        jukebox_res,
        video_res,
        ui_font,
    ): (
        Option<Res<AssetServer>>,
        Res<AssetRootResource>,
        Res<ModResource>,
        Res<SimWorldResource>,
        Res<ClientInteractionState>,
        Res<SoundSettingsResource>,
        Option<Res<JukeboxState>>,
        Option<Res<VideoPlaybackResource>>,
        Option<Res<UiFontResource>>,
    ),
    (mut ui_cache, mut subwindows, mut ui_blocker): (
        ResMut<UiAssetCache>,
        ResMut<PlayerSubWindowStack>,
        ResMut<UiInputBlocker>,
    ),
    (window_q, root_q, panel_q, mut video_img_q, mut video_prog_q, mut video_sub_q): (
        Query<&Window, With<PrimaryWindow>>,
        Query<Entity, With<PlayerSubWindowRoot>>,
        Query<(&PlayerSubWindowPanel, &ComputedNode, &GlobalTransform)>,
        Query<&mut ImageNode, With<BriefingVideoImageNode>>,
        Query<&mut Node, With<BriefingVideoProgressFill>>,
        Query<&mut Text, With<BriefingVideoSubtitleText>>,
    ),
) {
    let Ok(root_ent) = root_q.get_single() else {
        return;
    };

    let visible_in_player_mode = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open;
    if !visible_in_player_mode || subwindows.stack.is_empty() {
        ui_blocker.over_player_subwindow = false;
        subwindows.window_rects.clear();
        if subwindows.has_spawned_children {
            commands.entity(root_ent).despawn_descendants();
            subwindows.has_spawned_children = false;
            subwindows.last_rendered_sig.clear();
        }
        return;
    }

    let pid = interaction.local_player_id;
    let player_faction = sim_res
        .0
        .players
        .get(&pid)
        .map(|p| p.faction.clone())
        .unwrap_or_else(|| "allies".to_string());

    let jb_sig = jukebox_res
        .as_deref()
        .map(|j| {
            format!(
                "{:?}:{}:{}:{}:{}:{:?}:{}",
                j.current_index,
                j.is_playing,
                j.shuffle,
                j.repeat_one,
                j.dynamic_combat_mode,
                j.faction_filter,
                j.tracks.len()
            )
        })
        .unwrap_or_default();
    let cmd_tree_sig = format!(
        "{:?}:{:?}",
        sim_res.0.unlocked_commander_nodes.get(&pid),
        sim_res.0.players.get(&pid).map(|p| (
            p.commander_rank,
            p.promotion_points,
            p.commander_xp as u32
        ))
    );

    ui_cache.ensure_disk_catalogs(&asset_root.0);
    let sig = format!(
        "stack:{:?}|fac:{}|tech:{:?}|opt_tab:{}|rebind:{:?}|hk:{:?}|grid:{}|wasd:{}|ssao:{}|shd:{}|wm:{:?}|res:{:?}|vs:{}|fps:{}|lang:{}|as:{}:{}|saves:{}:{}|eva_len:{}|obj:{:?}:{}|jb:{}:{}|ct:{}|alliances:{:?}|entities:{}",
        subwindows.stack,
        player_faction,
        subwindows.inspected_tech_id,
        interaction.active_options_tab,
        interaction.rebinding_hotkey_slot,
        interaction.hotkeys,
        interaction.grid_hotkeys_enabled,
        interaction.wasd_pan_enabled,
        interaction.ssao_quality,
        interaction.shadow_quality,
        interaction.window_mode,
        interaction.resolution,
        interaction.vsync_enabled,
        interaction.fps_cap,
        interaction.language,
        interaction.autosave_interval_secs != 0,
        interaction.autosave_interval_secs,
        ui_cache.save_files.len(),
        interaction.save_slot_name_input,
        interaction.eva_log_archive.len(),
        sim_res.0.mission_objective,
        sim_res.0.mission_objectives.len(),
        jb_sig,
        interaction.jukebox_page,
        cmd_tree_sig,
        sim_res.0.players.get(&pid).map(|p| (&p.allied_players, p.credits)),
        sim_res.0.entities.len() / 4,
    );

    if subwindows.last_rendered_sig != sig {
        subwindows.last_rendered_sig = sig;
        commands.entity(root_ent).despawn_descendants();
        subwindows.has_spawned_children = true;

        let (panel_bg, header_bg, border_col) =
            resolve_faction_window_palette(&mod_res.0, &player_faction);
        let stack_copy = subwindows.stack.clone();
        let inspected_tech = subwindows.inspected_tech_id.clone();

        commands.entity(root_ent).with_children(|root| {
            for (z_idx, kind) in stack_copy.iter().copied().enumerate() {
                let (base_left, base_top, width_px) = kind.default_position();
                let cascade = (z_idx as f32) * 14.0;
                root.spawn((
                    PlayerSubWindowPanel(kind),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(base_left + cascade),
                        top: Val::Px(base_top + cascade),
                        width: Val::Px(width_px),
                        max_height: Val::Px(560.0),
                        flex_direction: FlexDirection::Column,
                        border: UiRect::all(Val::Px(2.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(panel_bg),
                    BorderColor(border_col),
                    ZIndex(30 + z_idx as i32),
                ))
                .with_children(|win| {
                    // Mod-skinned title bar + [X] Close button
                    win.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(Val::Px(1.5)),
                            ..default()
                        },
                        BackgroundColor(header_bg),
                        BorderColor(border_col),
                    ))
                    .with_children(|hdr| {
                        hdr.spawn((
                            Text::new(sanitize_ui_text(kind.title())),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.92, 0.62)),
                        ));
                        spawn_styled_button(
                            hdr,
                            "X",
                            SidebarAction::ClosePlayerWindow(kind),
                            Color::srgb(0.52, 0.14, 0.16),
                            Val::Px(28.0),
                        );
                    });

                    // Window content body
                    win.spawn(Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|body| match kind {
                        PlayerSubWindowKind::Briefing => {
                            briefing::spawn_briefing_window(
                                body,
                                &mod_res.0,
                                &sim_res.0,
                                &interaction,
                                video_res.as_deref(),
                            );
                        }
                        PlayerSubWindowKind::Diplomacy => {
                            diplomacy::spawn_diplomacy_window(
                                body,
                                &mod_res.0,
                                &sim_res.0,
                                &interaction,
                            );
                        }
                        PlayerSubWindowKind::TechTree => {
                            tech_tree::spawn_tech_tree_window(
                                body,
                                asset_server.as_deref(),
                                &asset_root,
                                &mut ui_cache,
                                &mod_res.0,
                                &sim_res.0,
                                pid,
                                &player_faction,
                                inspected_tech.as_deref(),
                            );
                        }
                        PlayerSubWindowKind::Logbook => {
                            logbook::spawn_logbook_window(
                                body,
                                &mod_res.0,
                                &sim_res.0,
                                &interaction,
                            );
                        }
                        PlayerSubWindowKind::Options => {
                            options::spawn_options_window(
                                body,
                                ui_font.as_deref(),
                                &mod_res.0,
                                &interaction,
                                &sound_res,
                                border_col,
                            );
                        }
                        PlayerSubWindowKind::SaveLoad => {
                            let saves_clone = ui_cache.save_files.clone();
                            save_load::spawn_save_load_window(
                                body,
                                ui_font.as_deref(),
                                &mod_res.0,
                                &interaction,
                                &saves_clone,
                            );
                        }
                        PlayerSubWindowKind::Jukebox => {
                            jukebox::spawn_jukebox_window(
                                body,
                                &interaction,
                                jukebox_res.as_deref(),
                                border_col,
                            );
                        }
                        PlayerSubWindowKind::CommanderTree => {
                            commander_tree::spawn_commander_tree_window(
                                body,
                                &mod_res.0,
                                &sim_res.0,
                                &interaction,
                                pid,
                                &player_faction,
                                border_col,
                            );
                        }
                    });
                });
            }
        });
    }

    // Live-stream FMV texture, progress bar, and subtitle inside Briefing window without rebuilding DOM
    if let Some(vid) = video_res.as_deref() {
        for mut img_node in &mut video_img_q {
            if img_node.image != vid.frame_image {
                img_node.image = vid.frame_image.clone();
            }
        }
        let pct = (vid.progress_01() * 100.0).clamp(0.0, 100.0);
        for mut fill_node in &mut video_prog_q {
            fill_node.width = Val::Percent(pct);
        }
        let sub_str = vid.current_subtitle.as_deref().unwrap_or("").trim();
        for mut sub_txt in &mut video_sub_q {
            let status = if vid.is_playing {
                if vid.paused {
                    "PAUSED"
                } else {
                    "PLAYING"
                }
            } else {
                "IDLE"
            };
            let caption = if sub_str.is_empty() {
                format!(
                    "[{status}] {} ({:.1}s / {:.1}s)",
                    vid.active_title,
                    vid.elapsed_secs,
                    vid.duration_secs.max(0.1)
                )
            } else {
                format!(
                    "{sub_str}  [{:.1}s / {:.1}s]",
                    vid.elapsed_secs,
                    vid.duration_secs.max(0.1)
                )
            };
            **sub_txt = sanitize_ui_text(&caption);
        }
    }

    // Recompute logical window hit-test rectangles and update UiInputBlocker
    if let Ok(window) = window_q.get_single() {
        let sf = window.scale_factor();
        subwindows.window_rects.clear();
        for (panel, computed, gt) in &panel_q {
            let rect = ui_node_logical_rect(computed, gt, sf);
            if rect.width() > 2.0 && rect.height() > 2.0 {
                subwindows.window_rects.insert(panel.0, rect);
            }
        }
        ui_blocker.over_player_subwindow = window
            .cursor_position()
            .is_some_and(|cursor| subwindows.contains_pointer(cursor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_closes_topmost_subwindow_in_lifo_order_before_pause_menu() {
        let mut stack = PlayerSubWindowStack::default();
        stack.open(PlayerSubWindowKind::Logbook);
        stack.open(PlayerSubWindowKind::TechTree);
        stack.open(PlayerSubWindowKind::Diplomacy);
        // Re-opening Logbook brings it to top
        stack.open(PlayerSubWindowKind::Logbook);

        assert_eq!(stack.close_topmost(), Some(PlayerSubWindowKind::Logbook));
        assert_eq!(stack.close_topmost(), Some(PlayerSubWindowKind::Diplomacy));
        assert_eq!(stack.close_topmost(), Some(PlayerSubWindowKind::TechTree));
        assert_eq!(stack.close_topmost(), None);
    }

    #[test]
    fn pointer_over_open_subwindow_blocks_world_clicks() {
        let mut stack = PlayerSubWindowStack::default();
        stack.open(PlayerSubWindowKind::TechTree);
        stack.window_rects.insert(
            PlayerSubWindowKind::TechTree,
            Rect::from_corners(Vec2::new(80.0, 60.0), Vec2::new(760.0, 500.0)),
        );
        assert!(stack.contains_pointer(Vec2::new(320.0, 220.0)));
        assert!(!stack.contains_pointer(Vec2::new(20.0, 20.0)));
    }

    #[test]
    fn close_all_retains_has_spawned_children_flag_for_despawn() {
        let mut stack = PlayerSubWindowStack::default();
        stack.open(PlayerSubWindowKind::TechTree);
        stack.has_spawned_children = true;
        stack.last_rendered_sig = "stack:[TechTree]".to_string();

        stack.close_all();
        assert!(stack.stack.is_empty());
        assert!(stack.has_spawned_children);
    }
}
