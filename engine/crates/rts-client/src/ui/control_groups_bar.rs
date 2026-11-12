use crate::input::{AppPhase, ClientInteractionState, UiInputBlocker};
use crate::ui::studio_layout::sanitize_ui_text;
use crate::ui::widgets::ui_node_logical_rect;
use crate::ui::{ButtonVisualStyle, SidebarAction};
use crate::SimWorldResource;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[derive(Component)]
pub struct ControlGroupsBarRoot;

#[derive(Component, Clone, Copy)]
pub struct ControlGroupSlotButton(pub usize);

#[derive(Component, Clone, Copy)]
pub struct ControlGroupSlotText(pub usize);

pub fn control_group_slot_key_label(idx: usize) -> &'static str {
    match idx {
        0 => "1",
        1 => "2",
        2 => "3",
        3 => "4",
        4 => "5",
        5 => "6",
        6 => "7",
        7 => "8",
        8 => "9",
        _ => "0",
    }
}

pub fn spawn_control_groups_bar(commands: &mut Commands) {
    commands
        .spawn((
            ControlGroupsBarRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(10.0),
                display: Display::None,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.06, 0.11, 0.86)),
            BorderColor(Color::srgba(0.26, 0.48, 0.72, 0.72)),
            ZIndex(22),
        ))
        .with_children(|bar| {
            for idx in 0..10 {
                let key_lbl = control_group_slot_key_label(idx);
                bar.spawn((
                    ControlGroupSlotButton(idx),
                    Button,
                    SidebarAction::SelectControlGroup(idx),
                    ButtonVisualStyle {
                        normal: Color::srgba(0.08, 0.12, 0.19, 0.92),
                        hovered: Color::srgb(0.20, 0.36, 0.56),
                        pressed: Color::srgb(0.12, 0.24, 0.40),
                        default_border: Color::srgba(0.28, 0.42, 0.60, 0.65),
                    },
                    Node {
                        min_width: Val::Px(34.0),
                        height: Val::Px(24.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.12, 0.19, 0.92)),
                    BorderColor(Color::srgba(0.28, 0.42, 0.60, 0.65)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        ControlGroupSlotText(idx),
                        Text::new(format!("[{key_lbl}]")),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.62, 0.72, 0.84, 0.78)),
                    ));
                });
            }
        });
}

#[allow(clippy::type_complexity)]
pub fn update_control_groups_bar(
    interaction: Res<ClientInteractionState>,
    sim_res: Res<SimWorldResource>,
    mut ui_blocker: ResMut<UiInputBlocker>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut root_q: Query<
        (&mut Node, &ComputedNode, &GlobalTransform),
        (With<ControlGroupsBarRoot>, Without<ControlGroupSlotButton>),
    >,
    mut slot_btn_q: Query<
        (
            &ControlGroupSlotButton,
            &Interaction,
            &mut ButtonVisualStyle,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<ControlGroupsBarRoot>,
    >,
    mut slot_txt_q: Query<(&ControlGroupSlotText, &mut Text, &mut TextColor)>,
) {
    let show = interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open
        && !interaction.replay_active;

    let Ok((mut root_node, computed, gt)) = root_q.get_single_mut() else {
        return;
    };

    root_node.display = if show { Display::Flex } else { Display::None };
    if !show {
        return;
    }

    if let Ok(window) = window_q.get_single() {
        if let Some(cursor) = window.cursor_position() {
            let rect = ui_node_logical_rect(computed, gt, window.scale_factor());
            if rect.width() > 2.0 && rect.height() > 2.0 && rect.contains(cursor) {
                ui_blocker.over_player_hud = true;
            }
        }
    }

    let pid = interaction.local_player_id;
    let mut alive_counts = [0usize; 10];
    let mut is_active_group = [false; 10];
    for idx in 0..10 {
        let grp = &interaction.control_groups[idx];
        let alive: Vec<u64> = grp
            .iter()
            .copied()
            .filter(|id| {
                sim_res
                    .0
                    .entities
                    .get(id)
                    .is_some_and(|e| e.player_id == pid && e.transported_by.is_none())
            })
            .collect();
        alive_counts[idx] = alive.len();
        is_active_group[idx] = !alive.is_empty()
            && alive.len() == interaction.selected_ids.len()
            && alive.iter().all(|id| interaction.selected_ids.contains(id));
    }

    for (slot, btn_inter, mut style, mut bg, mut border) in &mut slot_btn_q {
        let idx = slot.0.min(9);
        let cnt = alive_counts[idx];
        let selected = is_active_group[idx];
        let (normal, default_border) = if selected {
            (
                Color::srgba(0.18, 0.44, 0.34, 0.96),
                Color::srgb(0.42, 0.96, 0.66),
            )
        } else if cnt > 0 {
            (
                Color::srgba(0.12, 0.24, 0.38, 0.95),
                Color::srgb(0.44, 0.76, 0.98),
            )
        } else {
            (
                Color::srgba(0.06, 0.09, 0.14, 0.72),
                Color::srgba(0.22, 0.32, 0.46, 0.48),
            )
        };
        style.normal = normal;
        style.default_border = default_border;
        if *btn_inter == Interaction::None {
            *bg = BackgroundColor(normal);
            *border = BorderColor(default_border);
        }
    }

    for (slot, mut txt, mut col) in &mut slot_txt_q {
        let idx = slot.0.min(9);
        let key_lbl = control_group_slot_key_label(idx);
        let cnt = alive_counts[idx];
        let label = if cnt > 0 {
            format!("[{key_lbl}] {cnt}")
        } else {
            format!("[{key_lbl}]")
        };
        let sanitized = sanitize_ui_text(&label);
        if **txt != sanitized {
            **txt = sanitized;
        }
        *col = if is_active_group[idx] {
            TextColor(Color::srgb(0.98, 0.95, 0.56))
        } else if cnt > 0 {
            TextColor(Color::srgb(0.90, 0.96, 1.0))
        } else {
            TextColor(Color::srgba(0.55, 0.64, 0.76, 0.65))
        };
    }
}
