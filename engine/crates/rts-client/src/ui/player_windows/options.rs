use super::options_hotkeys::spawn_hotkeys_controls_tab;
use crate::audio::SoundSettingsResource;
use crate::input::ClientInteractionState;
use crate::rendering::{ShadowQualitySetting, SsaoQualitySetting};
use crate::settings_store::ClientWindowMode;
use crate::ui::locale::ui_t;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button, UiFontResource};
use crate::ui::widgets::{spawn_value_slider, UiSliderTarget};
use crate::ui::SidebarAction;
use bevy::prelude::*;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::AudioBusKind;

pub fn spawn_options_window(
    parent: &mut ChildBuilder,
    ui_font: Option<&UiFontResource>,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    sound_res: &SoundSettingsResource,
    accent: Color,
) {
    // 4-Tab Bar: Graphics & Display | 5-Bus Audio | Controls & Hotkeys (30) | Gameplay & Language
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(5.0),
            ..default()
        })
        .with_children(|tabs| {
            let tab_defs = [
                (0u8, ui_t(loaded, interaction, "ui.options.graphics")),
                (1u8, ui_t(loaded, interaction, "ui.options.audio")),
                (2u8, ui_t(loaded, interaction, "ui.options.hotkeys")),
                (3u8, ui_t(loaded, interaction, "ui.options.gameplay")),
            ];
            for (idx, lbl) in tab_defs {
                let active = interaction.active_options_tab == idx;
                spawn_styled_button(
                    tabs,
                    &lbl,
                    SidebarAction::SetOptionsTab(idx),
                    if active {
                        Color::srgb(0.24, 0.44, 0.64)
                    } else {
                        Color::srgb(0.14, 0.20, 0.30)
                    },
                    Val::Percent(25.0),
                );
            }
        });

    match interaction.active_options_tab {
        0 => {
            // Live Window Mode & Resolution Controls (`saves/settings.toml`)
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_styled_button(
                        row,
                        &format!(
                            "WINDOW: {}",
                            ClientWindowMode::from_str_lossy(&interaction.window_mode).label()
                        ),
                        SidebarAction::CycleWindowMode,
                        Color::srgb(0.20, 0.34, 0.50),
                        Val::Percent(50.0),
                    );
                    let vsync_lbl = if interaction.vsync_enabled {
                        "VSYNC: ON (AutoVsync)"
                    } else {
                        "VSYNC: OFF (Immediate)"
                    };
                    spawn_styled_button(
                        row,
                        vsync_lbl,
                        SidebarAction::ToggleVsync,
                        if interaction.vsync_enabled {
                            Color::srgb(0.20, 0.44, 0.32)
                        } else {
                            Color::srgb(0.32, 0.24, 0.20)
                        },
                        Val::Percent(50.0),
                    );
                });

            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    spawn_styled_button(
                        row,
                        "< RES",
                        SidebarAction::CycleResolution(false),
                        Color::srgb(0.18, 0.28, 0.42),
                        Val::Px(72.0),
                    );
                    spawn_styled_button(
                        row,
                        &format!(
                            "RESOLUTION: {}x{}",
                            interaction.resolution[0], interaction.resolution[1]
                        ),
                        SidebarAction::CycleResolution(true),
                        Color::srgb(0.16, 0.32, 0.48),
                        Val::Percent(48.0),
                    );
                    spawn_styled_button(
                        row,
                        "RES >",
                        SidebarAction::CycleResolution(true),
                        Color::srgb(0.18, 0.28, 0.42),
                        Val::Px(72.0),
                    );
                    let fps_lbl = if interaction.fps_cap == 0 {
                        "FPS: UNLIMITED".to_string()
                    } else {
                        format!("FPS CAP: {}", interaction.fps_cap)
                    };
                    spawn_styled_button(
                        row,
                        &fps_lbl,
                        SidebarAction::CycleFpsCap,
                        Color::srgb(0.22, 0.30, 0.44),
                        Val::Percent(32.0),
                    );
                });

            spawn_value_slider(
                parent,
                ui_font,
                "Global UI / DPI Scale (75% - 150%, saved to saves/settings.toml)",
                UiSliderTarget::UiScale,
                0.75,
                1.50,
                interaction.ui_scale,
                accent,
            );
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    for (pct, lbl) in [
                        (85, "85%"),
                        (100, "100% (Default)"),
                        (125, "125%"),
                        (140, "140%"),
                    ] {
                        spawn_styled_button(
                            row,
                            lbl,
                            SidebarAction::SetUiScalePercent(pct),
                            Color::srgb(0.18, 0.28, 0.42),
                            Val::Percent(25.0),
                        );
                    }
                });

            parent.spawn((
                Text::new(sanitize_ui_text("Directional Cascade Shadow Quality:")),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.84, 0.92, 0.98)),
            ));
            let active_shadow = ShadowQualitySetting::from_str_lossy(&interaction.shadow_quality);
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    for q in ShadowQualitySetting::ALL {
                        let selected = q == active_shadow;
                        spawn_styled_button(
                            row,
                            q.label(),
                            SidebarAction::SetShadowQuality(q.as_str().to_string()),
                            if selected {
                                Color::srgb(0.22, 0.48, 0.34)
                            } else {
                                Color::srgb(0.16, 0.24, 0.36)
                            },
                            Val::Percent(25.0),
                        );
                    }
                });

            parent.spawn((
                Text::new(sanitize_ui_text(
                    "Screen-Space Ambient Occlusion (SSAO Quality):",
                )),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.84, 0.92, 0.98)),
            ));
            let active_ssao = SsaoQualitySetting::from_str_lossy(&interaction.ssao_quality);
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    for q in SsaoQualitySetting::ALL {
                        let selected = q == active_ssao;
                        spawn_styled_button(
                            row,
                            q.label(),
                            SidebarAction::SetSsaoQuality(q.as_str().to_string()),
                            if selected {
                                Color::srgb(0.22, 0.48, 0.34)
                            } else {
                                Color::srgb(0.16, 0.24, 0.36)
                            },
                            Val::Percent(33.3),
                        );
                    }
                });

            spawn_styled_button(
                parent,
                &format!(
                    "Atmosphere & Lighting Preset: {} (Click to Cycle)",
                    interaction.environment_preset.label()
                ),
                SidebarAction::CycleEnvironmentPreset,
                Color::srgb(0.20, 0.34, 0.46),
                Val::Percent(100.0),
            );
        }
        1 => {
            let mute_lbl = if sound_res.enabled {
                "Audio Output: ENABLED (Click to Mute [Shift+F9])"
            } else {
                "Audio Output: MUTED (Click to Unmute [Shift+F9])"
            };
            spawn_styled_button(
                parent,
                mute_lbl,
                SidebarAction::ToggleAudioMute,
                if sound_res.enabled {
                    Color::srgb(0.18, 0.40, 0.26)
                } else {
                    Color::srgb(0.46, 0.18, 0.18)
                },
                Val::Percent(100.0),
            );
            let buses = [
                (AudioBusKind::Master, "Master Bus Volume"),
                (AudioBusKind::Music, "Soundtrack & Jukebox Bus"),
                (AudioBusKind::CombatSfx, "3D Spatial Combat SFX Bus"),
                (AudioBusKind::UnitVoice, "Unit Acknowledgements Bus"),
                (AudioBusKind::Eva, "EVA Tactical Announcer Bus"),
            ];
            for (bus, title) in buses {
                spawn_value_slider(
                    parent,
                    ui_font,
                    title,
                    UiSliderTarget::AudioBus(bus),
                    0.0,
                    1.0,
                    sound_res.bus_volume(bus),
                    accent,
                );
            }
        }
        2 => {
            spawn_hotkeys_controls_tab(parent, ui_font, interaction, accent);
        }
        _ => {
            parent.spawn((
                Text::new(sanitize_ui_text(&format!(
                    "Commander Profile: {} (ID: {})",
                    interaction.player_display_name, interaction.player_identity_short
                ))),
                TextFont {
                    font_size: 11.5,
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.88, 0.48)),
            ));

            parent.spawn((
                Text::new(sanitize_ui_text("Live UI & Mod Localization Language:")),
                TextFont {
                    font_size: 11.5,
                    ..default()
                },
                TextColor(Color::srgb(0.86, 0.92, 0.98)),
            ));
            let is_de = interaction.language.eq_ignore_ascii_case("de")
                || interaction.language.to_ascii_lowercase().starts_with("de-");
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_styled_button(
                        row,
                        "ENGLISH (EN)",
                        SidebarAction::SetLanguage("en".to_string()),
                        if !is_de {
                            Color::srgb(0.22, 0.48, 0.34)
                        } else {
                            Color::srgb(0.16, 0.24, 0.36)
                        },
                        Val::Percent(50.0),
                    );
                    spawn_styled_button(
                        row,
                        "DEUTSCH (DE)",
                        SidebarAction::SetLanguage("de".to_string()),
                        if is_de {
                            Color::srgb(0.22, 0.48, 0.34)
                        } else {
                            Color::srgb(0.16, 0.24, 0.36)
                        },
                        Val::Percent(50.0),
                    );
                });

            let edge_lbl = if interaction.edge_scroll_enabled {
                "Mouse Screen-Edge Camera Pan: ENABLED [ScrollLock]"
            } else {
                "Mouse Screen-Edge Camera Pan: DISABLED [ScrollLock]"
            };
            spawn_styled_button(
                parent,
                edge_lbl,
                SidebarAction::ToggleEdgeScroll,
                Color::srgb(0.20, 0.32, 0.44),
                Val::Percent(100.0),
            );
            let fow_lbl = if interaction.fog_of_war_enabled {
                "Tactical Fog of War: ENABLED [F4]"
            } else {
                "Tactical Fog of War: DISABLED (Full Vision) [F4]"
            };
            spawn_styled_button(
                parent,
                fow_lbl,
                SidebarAction::ToggleFogOfWar,
                Color::srgb(0.22, 0.34, 0.30),
                Val::Percent(100.0),
            );
            spawn_styled_button(
                parent,
                &format!(
                    "Offline Simulation Speed: {:.1}x [F6]",
                    interaction.sim_speed_multiplier
                ),
                SidebarAction::CycleGameSpeed,
                Color::srgb(0.26, 0.30, 0.42),
                Val::Percent(100.0),
            );
            let autosave_lbl = if interaction.autosave_interval_secs == 0 {
                "Rotating Autosave (autosave_1..3.rtssav): OFF".to_string()
            } else {
                format!(
                    "Rotating Autosave (autosave_1..3.rtssav): Every {} min",
                    (interaction.autosave_interval_secs / 60).max(1)
                )
            };
            spawn_styled_button(
                parent,
                &autosave_lbl,
                SidebarAction::CycleAutosaveInterval,
                Color::srgb(0.22, 0.36, 0.42),
                Val::Percent(100.0),
            );

            let active_hud = interaction.effective_hud_layout(loaded);
            let hud_lbl = match active_hud {
                rts_sdk::manifest::HudLayoutPreset::ClassicRightSidebar => {
                    "Classic Right Sidebar (C&C / RA2)"
                }
                rts_sdk::manifest::HudLayoutPreset::ModernBottomCommandCard => {
                    "Modern Bottom Command Card (SC2 / WC3 4x3)"
                }
                rts_sdk::manifest::HudLayoutPreset::CompactBarOverlay => {
                    "Compact Tactical Overlay (SupCom / BAR)"
                }
            };
            let ov_tag = if interaction.hud_layout_override.is_some() {
                "Custom Override"
            } else {
                "Mod Default"
            };
            spawn_styled_button(
                parent,
                &format!("HUD Layout Preset: {hud_lbl} [{ov_tag}]"),
                SidebarAction::CycleHudLayoutPreset,
                Color::srgb(0.20, 0.38, 0.48),
                Val::Percent(100.0),
            );

            let cb_mode = crate::rendering::ColorblindPaletteMode::from_str_lossy(
                &interaction.colorblind_mode,
            );
            spawn_styled_button(
                parent,
                &format!("Colorblind & Team Accessibility: {}", cb_mode.label()),
                SidebarAction::CycleColorblindMode,
                Color::srgb(0.26, 0.30, 0.48),
                Val::Percent(100.0),
            );

            spawn_styled_button(
                parent,
                if interaction.smart_cast_enabled {
                    "Smart-Cast Abilities (Instant Cast at Cursor): ENABLED"
                } else {
                    "Smart-Cast Abilities (Two-Step Aim & Click): DISABLED"
                },
                SidebarAction::ToggleSmartCast,
                if interaction.smart_cast_enabled {
                    Color::srgb(0.20, 0.44, 0.32)
                } else {
                    Color::srgb(0.22, 0.26, 0.36)
                },
                Val::Percent(100.0),
            );

            spawn_styled_button(
                parent,
                if interaction.auto_resync_on_desync {
                    "Multiplayer Desync Recovery: AUTO-RESYNC FROM HOST"
                } else {
                    "Multiplayer Desync Recovery: MANUAL [RESYNC] PROMPT"
                },
                SidebarAction::ToggleAutoResyncOnDesync,
                Color::srgb(0.24, 0.34, 0.42),
                Val::Percent(100.0),
            );
        }
    }
}
