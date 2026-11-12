use crate::audio::{SoundEffectKind, SoundSettingsResource};
use crate::input::{ClientInteractionState, UiInputBlocker};
use crate::ui::studio_layout::UiFontResource;
use crate::ui::LastSidebarSignature;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::schema::AudioBusKind;

/// Converts a Bevy 0.15 `ComputedNode` + `GlobalTransform` (physical target pixels) into logical
/// window coordinates matching `Window::cursor_position()`, accurately accounting for `UiScale`.
pub fn ui_node_logical_rect(
    computed: &ComputedNode,
    gt: &GlobalTransform,
    window_scale_factor: f32,
) -> Rect {
    let sf = window_scale_factor.max(0.001);
    let center = gt.translation().truncate() / sf;
    let half = (computed.size() / sf) * 0.5;
    Rect::from_center_half_size(center, half)
}

/// Mouse-wheel scrollable viewport container for `bevy_ui` lists and logs.
#[derive(Component, Debug, Clone)]
pub struct UiScrollContainer {
    pub offset_px: f32,
    pub max_scroll_px: f32,
    pub step_px: f32,
}

impl UiScrollContainer {
    pub fn new(max_scroll_px: f32) -> Self {
        Self {
            offset_px: 0.0,
            max_scroll_px: max_scroll_px.max(0.0),
            step_px: 34.0,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct UiScrollContent;

/// Identifies which persistent client setting or audio bus a draggable `UiValueSlider` controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiSliderTarget {
    UiScale,
    CameraPanSpeed,
    AudioBus(AudioBusKind),
}

#[derive(Component, Debug, Clone)]
pub struct UiValueSlider {
    pub target: UiSliderTarget,
    pub min: f32,
    pub max: f32,
    pub value: f32,
    pub dragging: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct UiValueSliderFill(pub UiSliderTarget);

#[derive(Component, Debug, Clone, Copy)]
pub struct UiValueSliderLabel(pub UiSliderTarget);

/// Dropdown selector widget for `bevy_ui` menus and sub-windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiDropdownTarget {
    AiDifficulty,
    AiPersonality,
    EnvironmentLighting,
}

#[derive(Component, Debug, Clone)]
pub struct UiDropdown {
    pub target: UiDropdownTarget,
    pub options: Vec<String>,
    pub selected_index: usize,
    pub expanded: bool,
}

/// Focusable single-line text input widget for `bevy_ui` screens (Multiplayer IP, Seed, Chat/Taunt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiTextFieldTarget {
    ServerAddress,
    LobbyServerUrl,
    ProceduralSeed,
    CommanderName,
    LobbyPassword,
    StagingChat,
    SaveSlotName,
    WorkshopSearch,
}

#[derive(Component, Debug, Clone)]
pub struct UiTextInput {
    pub target: UiTextFieldTarget,
    pub allow_letters: bool,
    pub focused: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct UiTextInputDisplay(pub UiTextFieldTarget);

#[allow(clippy::too_many_arguments)]
pub fn spawn_value_slider(
    parent: &mut ChildBuilder,
    ui_font: Option<&UiFontResource>,
    label: &str,
    target: UiSliderTarget,
    min: f32,
    max: f32,
    value: f32,
    accent: Color,
) {
    let clamped = value.clamp(min, max);
    let pct = ((clamped - min) / (max - min).max(0.0001)).clamp(0.0, 1.0) * 100.0;
    let formatted = format_slider_value(target, clamped);
    let font_handle = ui_font.map(|f| f.font.clone()).unwrap_or_default();

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            margin: UiRect::vertical(Val::Px(3.0)),
            ..default()
        })
        .with_children(|col| {
            col.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|hdr| {
                hdr.spawn((
                    Text::new(label.to_string()),
                    TextFont {
                        font: font_handle.clone(),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.90, 0.95)),
                ));
                hdr.spawn((
                    Text::new(formatted),
                    TextFont {
                        font: font_handle.clone(),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.84, 0.34)),
                    UiValueSliderLabel(target),
                ));
            });

            col.spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(14.0),
                    border: UiRect::all(Val::Px(1.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.96)),
                BorderColor(Color::srgba(0.38, 0.48, 0.62, 0.85)),
                UiValueSlider {
                    target,
                    min,
                    max,
                    value: clamped,
                    dragging: false,
                },
            ))
            .with_children(|track| {
                track.spawn((
                    Node {
                        width: Val::Percent(pct),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(accent),
                    UiValueSliderFill(target),
                ));
            });
        });
}

pub fn spawn_text_input(
    parent: &mut ChildBuilder,
    ui_font: Option<&UiFontResource>,
    label: &str,
    target: UiTextFieldTarget,
    current_value: &str,
    allow_letters: bool,
    focused: bool,
) {
    let font_handle = ui_font.map(|f| f.font.clone()).unwrap_or_default();
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            margin: UiRect::vertical(Val::Px(3.0)),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font: font_handle.clone(),
                    font_size: 11.5,
                    ..default()
                },
                TextColor(Color::srgb(0.75, 0.82, 0.90)),
            ));
            let shown = if focused {
                format!("{current_value}|")
            } else {
                current_value.to_string()
            };
            col.spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(28.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if focused {
                    Color::srgba(0.12, 0.18, 0.28, 0.98)
                } else {
                    Color::srgba(0.06, 0.08, 0.12, 0.94)
                }),
                BorderColor(if focused {
                    Color::srgb(0.98, 0.80, 0.25)
                } else {
                    Color::srgba(0.36, 0.45, 0.58, 0.85)
                }),
                UiTextInput {
                    target,
                    allow_letters,
                    focused,
                },
            ))
            .with_children(|box_node| {
                box_node.spawn((
                    Text::new(shown),
                    TextFont {
                        font: font_handle,
                        font_size: 12.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.97, 1.0)),
                    UiTextInputDisplay(target),
                ));
            });
        });
}

fn format_slider_value(target: UiSliderTarget, val: f32) -> String {
    match target {
        UiSliderTarget::UiScale => format!("{:.0}%", val * 100.0),
        UiSliderTarget::CameraPanSpeed => format!("{:.0} u/s", val),
        UiSliderTarget::AudioBus(_) => format!("{:.0}%", val * 100.0),
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_ui_widgets(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel_evr: EventReader<MouseWheel>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut interaction: ResMut<ClientInteractionState>,
    mut sound_res: ResMut<SoundSettingsResource>,
    mut ui_blocker: ResMut<UiInputBlocker>,
    mut last_sig: ResMut<LastSidebarSignature>,
    mut scroll_q: Query<(
        &mut UiScrollContainer,
        &ComputedNode,
        &GlobalTransform,
        &Children,
    )>,
    mut scroll_content_q: Query<&mut Node, (With<UiScrollContent>, Without<UiValueSliderFill>)>,
    mut slider_q: Query<(&mut UiValueSlider, &ComputedNode, &GlobalTransform)>,
    mut slider_fill_q: Query<(&mut Node, &UiValueSliderFill), Without<UiScrollContent>>,
    mut slider_label_q: Query<(&mut Text, &UiValueSliderLabel), Without<UiTextInputDisplay>>,
    mut text_input_q: Query<(
        &mut UiTextInput,
        &mut BorderColor,
        &ComputedNode,
        &GlobalTransform,
    )>,
    mut text_display_q: Query<(&mut Text, &UiTextInputDisplay), Without<UiValueSliderLabel>>,
) {
    let Ok(window) = window_q.get_single() else {
        return;
    };
    let cursor_opt = window.cursor_position();
    let win_sf = window.scale_factor();

    // 1. Mouse-wheel scrolling on hovered UiScrollContainer
    let mut wheel_y = 0.0;
    for ev in wheel_evr.read() {
        wheel_y += ev.y;
    }
    if wheel_y.abs() > 0.001 {
        if let Some(cursor) = cursor_opt {
            for (mut scroller, computed, gt, children) in &mut scroll_q {
                let rect = ui_node_logical_rect(computed, gt, win_sf);
                if rect.contains(cursor) {
                    let computed_h = (computed.size().y / win_sf).max(1.0);
                    let mut dynamic_max = scroller.max_scroll_px;
                    for &child in children.iter() {
                        if let Ok(c_node) = scroll_content_q.get_mut(child) {
                            let _ = c_node;
                        }
                    }
                    dynamic_max = dynamic_max.max(0.0);
                    let _ = computed_h;
                    scroller.offset_px =
                        (scroller.offset_px - wheel_y * scroller.step_px).clamp(0.0, dynamic_max);
                    for &child in children.iter() {
                        if let Ok(mut content_node) = scroll_content_q.get_mut(child) {
                            content_node.top = Val::Px(-scroller.offset_px);
                        }
                    }
                }
            }
        }
    }

    // 2. Draggable UiValueSlider tracks
    let mut settings_dirty = false;
    for (mut slider, computed, gt) in &mut slider_q {
        let rect = ui_node_logical_rect(computed, gt, win_sf);
        if mouse_buttons.just_pressed(MouseButton::Left) {
            if let Some(cursor) = cursor_opt {
                if rect.contains(cursor) {
                    slider.dragging = true;
                }
            }
        }
        if slider.dragging && mouse_buttons.pressed(MouseButton::Left) {
            if let Some(cursor) = cursor_opt {
                let t = ((cursor.x - rect.min.x) / rect.width().max(1.0)).clamp(0.0, 1.0);
                let new_val = slider.min + t * (slider.max - slider.min);
                slider.value = new_val;
                match slider.target {
                    UiSliderTarget::UiScale => {
                        interaction.ui_scale = new_val.clamp(0.75, 1.50);
                    }
                    UiSliderTarget::CameraPanSpeed => {
                        interaction.camera_pan_speed = new_val.clamp(12.0, 96.0);
                    }
                    UiSliderTarget::AudioBus(bus) => {
                        sound_res.set_bus_volume(bus, new_val.clamp(0.0, 1.0));
                    }
                }
                let pct = t * 100.0;
                for (mut fill_node, fill_tag) in &mut slider_fill_q {
                    if fill_tag.0 == slider.target {
                        fill_node.width = Val::Percent(pct);
                    }
                }
                let label_txt = format_slider_value(slider.target, slider.value);
                for (mut txt, lbl_tag) in &mut slider_label_q {
                    if lbl_tag.0 == slider.target {
                        **txt = label_txt.clone();
                    }
                }
            }
        }
        if slider.dragging && mouse_buttons.just_released(MouseButton::Left) {
            slider.dragging = false;
            settings_dirty = true;
        }
    }

    if settings_dirty {
        crate::settings_store::save_unified_client_settings(&sound_res, &interaction);
        sound_res.trigger(SoundEffectKind::UiClick);
    }

    // 3. Focusable UiTextInput fields
    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some(cursor) = cursor_opt {
            for (mut field, mut border, computed, gt) in &mut text_input_q {
                let rect = ui_node_logical_rect(computed, gt, win_sf);
                field.focused = rect.contains(cursor);
                *border = BorderColor(if field.focused {
                    Color::srgb(0.98, 0.80, 0.25)
                } else {
                    Color::srgba(0.36, 0.45, 0.58, 0.85)
                });
                if field.target == UiTextFieldTarget::ServerAddress {
                    interaction.ip_input_editing = field.focused;
                } else if field.target == UiTextFieldTarget::WorkshopSearch {
                    interaction.workshop_search_editing = field.focused;
                }
            }
        }
    }

    let mut any_focused = false;
    for (mut field, mut border, _, _) in &mut text_input_q {
        if !field.focused {
            continue;
        }
        if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
            field.focused = false;
            if field.target == UiTextFieldTarget::ServerAddress {
                interaction.ip_input_editing = false;
            } else if field.target == UiTextFieldTarget::WorkshopSearch {
                interaction.workshop_search_editing = false;
            } else if field.target == UiTextFieldTarget::CommanderName {
                crate::settings_store::save_unified_client_settings(&sound_res, &interaction);
            }
            *border = BorderColor(Color::srgba(0.36, 0.45, 0.58, 0.85));
            last_sig.studio_sig.clear();
            continue;
        }
        any_focused = true;
        match field.target {
            UiTextFieldTarget::ServerAddress => {
                append_widget_keys(
                    &keys,
                    &mut interaction.server_addr_input,
                    field.allow_letters,
                );
            }
            UiTextFieldTarget::LobbyServerUrl => {
                append_widget_keys(&keys, &mut interaction.lobby_server_url, true);
            }
            UiTextFieldTarget::ProceduralSeed => {
                let mut buf = interaction.procedural_map_seed.to_string();
                append_widget_keys(&keys, &mut buf, false);
                interaction.procedural_map_seed = buf.parse::<u64>().unwrap_or(0);
            }
            UiTextFieldTarget::CommanderName => {
                append_widget_keys(&keys, &mut interaction.player_display_name, true);
            }
            UiTextFieldTarget::LobbyPassword => {
                append_widget_keys(&keys, &mut interaction.lobby_password_input, true);
            }
            UiTextFieldTarget::StagingChat => {
                append_widget_keys(&keys, &mut interaction.staging_chat_input, true);
            }
            UiTextFieldTarget::SaveSlotName => {
                append_widget_keys(&keys, &mut interaction.save_slot_name_input, true);
            }
            UiTextFieldTarget::WorkshopSearch => {
                append_widget_keys(&keys, &mut interaction.workshop_search_query, true);
            }
        }
        let val_str = match field.target {
            UiTextFieldTarget::ServerAddress => format!("{}|", interaction.server_addr_input),
            UiTextFieldTarget::LobbyServerUrl => format!("{}|", interaction.lobby_server_url),
            UiTextFieldTarget::ProceduralSeed => format!("{}|", interaction.procedural_map_seed),
            UiTextFieldTarget::CommanderName => format!("{}|", interaction.player_display_name),
            UiTextFieldTarget::LobbyPassword => {
                format!(
                    "{}|",
                    "*".repeat(interaction.lobby_password_input.chars().count())
                )
            }
            UiTextFieldTarget::StagingChat => format!("{}|", interaction.staging_chat_input),
            UiTextFieldTarget::SaveSlotName => format!("{}|", interaction.save_slot_name_input),
            UiTextFieldTarget::WorkshopSearch => format!("{}|", interaction.workshop_search_query),
        };
        for (mut txt, tag) in &mut text_display_q {
            if tag.0 == field.target {
                **txt = val_str.clone();
            }
        }
    }
    ui_blocker.text_widget_focused = any_focused;
}

fn append_widget_keys(keys: &ButtonInput<KeyCode>, buf: &mut String, allow_letters: bool) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let digits = [
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
    ];
    for (k, ch) in digits {
        if keys.just_pressed(k) {
            buf.push(ch);
        }
    }
    if keys.just_pressed(KeyCode::Period) {
        buf.push(if shift { ':' } else { '.' });
    }
    if keys.just_pressed(KeyCode::Semicolon) && shift {
        buf.push(':');
    }
    if keys.just_pressed(KeyCode::Slash) {
        buf.push('/');
    }
    if keys.just_pressed(KeyCode::Minus) {
        buf.push(if shift { '_' } else { '-' });
    }
    if keys.just_pressed(KeyCode::Backspace) {
        buf.pop();
    }
    if allow_letters {
        if keys.just_pressed(KeyCode::Space) {
            buf.push(' ');
        }
        let letters = [
            (KeyCode::KeyA, 'a'),
            (KeyCode::KeyB, 'b'),
            (KeyCode::KeyC, 'c'),
            (KeyCode::KeyD, 'd'),
            (KeyCode::KeyE, 'e'),
            (KeyCode::KeyF, 'f'),
            (KeyCode::KeyG, 'g'),
            (KeyCode::KeyH, 'h'),
            (KeyCode::KeyI, 'i'),
            (KeyCode::KeyJ, 'j'),
            (KeyCode::KeyK, 'k'),
            (KeyCode::KeyL, 'l'),
            (KeyCode::KeyM, 'M'),
            (KeyCode::KeyN, 'n'),
            (KeyCode::KeyO, 'o'),
            (KeyCode::KeyP, 'p'),
            (KeyCode::KeyQ, 'q'),
            (KeyCode::KeyR, 'r'),
            (KeyCode::KeyS, 's'),
            (KeyCode::KeyT, 't'),
            (KeyCode::KeyU, 'u'),
            (KeyCode::KeyV, 'v'),
            (KeyCode::KeyW, 'w'),
            (KeyCode::KeyX, 'x'),
            (KeyCode::KeyY, 'y'),
            (KeyCode::KeyZ, 'z'),
        ];
        for (k, ch) in letters {
            if keys.just_pressed(k) {
                let low = ch.to_ascii_lowercase();
                buf.push(if shift { low.to_ascii_uppercase() } else { low });
            }
        }
    }
}
