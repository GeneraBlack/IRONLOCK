use crate::input::keymap::conflicting_hotkey_slots;
use crate::input::{ClientInteractionState, HotkeySlot};
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button, UiFontResource};
use crate::ui::widgets::{spawn_value_slider, UiScrollContainer, UiScrollContent, UiSliderTarget};
use crate::ui::SidebarAction;
use bevy::prelude::*;

pub fn spawn_hotkeys_controls_tab(
    parent: &mut ChildBuilder,
    ui_font: Option<&UiFontResource>,
    interaction: &ClientInteractionState,
    accent: Color,
) {
    spawn_value_slider(
        parent,
        ui_font,
        "Camera Pan Speed (Arrow Keys / WASD / Edge-Scroll)",
        UiSliderTarget::CameraPanSpeed,
        12.0,
        96.0,
        interaction.camera_pan_speed,
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
            let grid_lbl = if interaction.grid_hotkeys_enabled {
                "QWER/ASDF Grid Cameos: ON"
            } else {
                "QWER/ASDF Grid Cameos: OFF"
            };
            spawn_styled_button(
                row,
                grid_lbl,
                SidebarAction::ToggleGridHotkeys,
                if interaction.grid_hotkeys_enabled {
                    Color::srgb(0.20, 0.46, 0.30)
                } else {
                    Color::srgb(0.16, 0.24, 0.34)
                },
                Val::Percent(50.0),
            );
            let wasd_lbl = if interaction.wasd_pan_enabled {
                "WASD Camera Pan: ON"
            } else {
                "WASD Camera Pan: OFF"
            };
            spawn_styled_button(
                row,
                wasd_lbl,
                SidebarAction::ToggleWasdPan,
                if interaction.wasd_pan_enabled {
                    Color::srgb(0.20, 0.46, 0.30)
                } else {
                    Color::srgb(0.16, 0.24, 0.34)
                },
                Val::Percent(50.0),
            );
        });

    let conflicts = conflicting_hotkey_slots(&interaction.hotkeys);
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|hdr| {
            let badge = if conflicts.is_empty() {
                "30 Tactical Actions — Zero Key Conflicts".to_string()
            } else {
                format!(
                    "WARNING: {} Conflicting Keybinding(s) Detected!",
                    conflicts.len()
                )
            };
            hdr.spawn((
                Text::new(sanitize_ui_text(&badge)),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(if conflicts.is_empty() {
                    Color::srgb(0.58, 0.94, 0.70)
                } else {
                    Color::srgb(1.0, 0.45, 0.32)
                }),
            ));
            spawn_styled_button(
                hdr,
                "RESET DEFAULTS",
                SidebarAction::ResetHotkeysToDefaults,
                Color::srgb(0.42, 0.22, 0.16),
                Val::Px(138.0),
            );
        });

    let max_scroll = (HotkeySlot::ALL.len() as f32 * 30.0 - 210.0).max(0.0);
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(225.0),
                overflow: Overflow::clip_y(),
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.88)),
            BorderColor(Color::srgba(0.35, 0.52, 0.74, 0.7)),
            UiScrollContainer::new(max_scroll),
        ))
        .with_children(|vp| {
            vp.spawn((
                UiScrollContent,
                Node {
                    width: Val::Percent(100.0),
                    position_type: PositionType::Relative,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    ..default()
                },
            ))
            .with_children(|list| {
                for &slot in &HotkeySlot::ALL {
                    let key_str = interaction.binding_for(slot);
                    let waiting = interaction.rebinding_hotkey_slot == Some(slot);
                    let has_conflict = conflicts.contains(&slot);
                    let label = if waiting {
                        format!(
                            "[{}] {}: PRESS ANY KEY (ESC CANCEL)...",
                            slot.category(),
                            slot.label()
                        )
                    } else if has_conflict {
                        format!(
                            "[CONFLICT] [{}] {}: [{}]",
                            slot.category(),
                            slot.label(),
                            key_str
                        )
                    } else {
                        format!("[{}] {}: [{}]", slot.category(), slot.label(), key_str)
                    };
                    let color = if waiting {
                        Color::srgb(0.58, 0.40, 0.14)
                    } else if has_conflict {
                        Color::srgb(0.52, 0.18, 0.18)
                    } else {
                        Color::srgb(0.16, 0.24, 0.36)
                    };
                    spawn_styled_button(
                        list,
                        &label,
                        SidebarAction::StartRebindHotkey(slot),
                        color,
                        Val::Percent(100.0),
                    );
                }
            });
        });
}
