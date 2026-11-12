use crate::camera::RtsCameraRig;
use crate::input::{AppPhase, ClientInteractionState, ProductionSidebarTab, UiInputBlocker};
use crate::rendering::PreviewStageCamera;
use crate::ui::studio_layout::{sanitize_ui_text, spawn_styled_button};
use crate::ui::widgets::ui_node_logical_rect;
use crate::ui::SidebarAction;
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::BuildingRole;
use rts_simulation::world::{EntityKind, SimWorld};

pub const BOOTCAMP_CAMPAIGN_ID: &str = "bootcamp_campaign";
pub const BOOTCAMP_MISSION_ID: &str = "ra2_bootcamp_01";
pub const TUTORIAL_STEP_COUNT: usize = 8;

#[derive(Resource, Debug, Clone, Default)]
pub struct TutorialCoachState {
    pub active: bool,
    pub dismissed_for_mission: Option<String>,
    pub step_index: usize,
    pub completed: bool,
    pub baseline_cam_focus: Option<Vec3>,
    pub baseline_cam_distance: Option<f32>,
    pub last_rendered_sig: String,
}

impl TutorialCoachState {
    pub fn reset_and_activate(&mut self) {
        self.active = true;
        self.dismissed_for_mission = None;
        self.step_index = 0;
        self.completed = false;
        self.baseline_cam_focus = None;
        self.baseline_cam_distance = None;
        self.last_rendered_sig.clear();
    }
}

#[derive(Component)]
pub struct TutorialCoachCardRoot;

#[derive(Component)]
pub struct TutorialCoachCardBody;

#[derive(Component)]
pub struct TutorialSidebarArrowBanner;

pub fn tutorial_step_title_and_hint(lang: &str, step: usize) -> (&'static str, &'static str) {
    let de = lang.eq_ignore_ascii_case("de");
    match step {
        0 => (
            if de {
                "SCHRITT 1/8: KAMERA SCHWENKEN & ZOOMEN"
            } else {
                "STEP 1/8: PAN & ZOOM BATTLEFIELD CAMERA"
            },
            if de {
                "Bewege die Kamera mit Pfeiltasten / WASD / Bildschirmrand und zoome mit dem Mausrad."
            } else {
                "Pan the 3D camera using Arrow Keys / WASD / screen edges and zoom with the Mouse Wheel."
            },
        ),
        1 => (
            if de {
                "SCHRITT 2/8: EINHEITEN & MBF AUSWÄHLEN"
            } else {
                "STEP 2/8: BOX-SELECT YOUR MCV & ESCORT"
            },
            if de {
                "Halte [LMB] gedrückt und ziehe einen Auswahlrahmen um dein Mobiles Baufahrzeug (MBF)."
            } else {
                "Hold [LMB] and drag a selection box around your Mobile Construction Vehicle (MCV)."
            },
        ),
        2 => (
            if de {
                "SCHRITT 3/8: BAUHOF AUFBAUEN [G]"
            } else {
                "STEP 3/8: DEPLOY MCV INTO CONSTRUCTION YARD [G]"
            },
            if de {
                "Wähle dein MBF aus und drücke [G] (oder klicke auf ABILITY), um den Bauhof zu entfalten."
            } else {
                "With your MCV selected, press [G] (or click ABILITY) on flat ground to deploy your Construction Yard."
            },
        ),
        3 => (
            if de {
                "SCHRITT 4/8: KRAFTWERK BAUEN"
            } else {
                "STEP 4/8: CONSTRUCT A POWER PLANT"
            },
            if de {
                "Klicke rechts in der Seitenleiste auf das Kraftwerk-Cameo und platziere es neben deinem Bauhof."
            } else {
                "Click the Power Plant cameo in the right sidebar and place it on green valid ground near your base."
            },
        ),
        4 => (
            if de {
                "SCHRITT 5/8: ERZRAFFINERIE ERRICHTEN"
            } else {
                "STEP 5/8: BUILD AN ORE REFINERY"
            },
            if de {
                "Baue eine Erzraffinerie nahe dem goldenen Erzfeld — ein Sammler erscheint automatisch!"
            } else {
                "Construct an Ore Refinery near the golden ore patch — a Harvester deploys automatically!"
            },
        ),
        5 => (
            if de {
                "SCHRITT 6/8: KASERNE & INFANTERIE AUSBILDEN"
            } else {
                "STEP 6/8: BUILD BARRACKS & TRAIN INFANTRY"
            },
            if de {
                "Errichte eine Kaserne und bilde im Reiter [INF] mindestens einen Infanterie-Trupp aus."
            } else {
                "Construct a Barracks, switch to the [INF] sidebar tab, and train at least one infantry squad."
            },
        ),
        6 => (
            if de {
                "SCHRITT 7/8: KAMPFGRUPPE [STRG+1] BINDEN"
            } else {
                "STEP 7/8: BIND CONTROL GROUP [CTRL+1]"
            },
            if de {
                "Wähle deine Kampfeinheiten aus und drücke [Strg+1], um sie auf Kontrollgruppe [1] zu legen."
            } else {
                "Select your combat units and press [Ctrl+1] to bind them to Control Group [1] on the tactical bar."
            },
        ),
        _ => (
            if de {
                "SCHRITT 8/8: ANGRIFFSBEFEHL [X / A] ERTEILEN"
            } else {
                "STEP 8/8: ISSUE AN ATTACK-MOVE ORDER"
            },
            if de {
                "Drücke Angriffsbewegung ([X] oder ATK-MOVE) und klicke Richtung Feindlager im Nordosten!"
            } else {
                "Press Attack-Move ([X] or ATK-MOVE button) and click toward the hostile outpost to advance!"
            },
        ),
    }
}

fn player_owns_building_role(
    world: &SimWorld,
    loaded: &LoadedMod,
    pid: u8,
    role: BuildingRole,
) -> bool {
    world.entities.values().any(|e| {
        if e.player_id != pid || e.construction_progress < 0.99 {
            return false;
        }
        if let EntityKind::Building { ref building_id } = e.kind {
            loaded
                .buildings
                .get(building_id)
                .is_some_and(|b| b.role == role)
        } else {
            false
        }
    })
}

pub fn is_tutorial_step_satisfied(
    step: usize,
    world: &SimWorld,
    loaded: &LoadedMod,
    interaction: &ClientInteractionState,
    cam_moved_or_zoomed: bool,
) -> bool {
    let pid = interaction.local_player_id;
    match step {
        0 => cam_moved_or_zoomed,
        1 => !interaction.selected_ids.is_empty(),
        2 => player_owns_building_role(world, loaded, pid, BuildingRole::ConstructionYard),
        3 => player_owns_building_role(world, loaded, pid, BuildingRole::PowerPlant),
        4 => player_owns_building_role(world, loaded, pid, BuildingRole::Refinery),
        5 => {
            let has_barracks =
                player_owns_building_role(world, loaded, pid, BuildingRole::Barracks);
            let trained_or_queued = world.players.get(&pid).is_some_and(|p| p.units_built > 0)
                || world
                    .entities
                    .values()
                    .any(|e| e.player_id == pid && !e.production_queue.is_empty());
            has_barracks && trained_or_queued
        }
        6 => interaction.control_groups.iter().any(|g| !g.is_empty()),
        7 => {
            interaction.attack_move_mode
                || world
                    .entities
                    .values()
                    .any(|e| e.player_id == pid && e.attack_move_dest.is_some())
        }
        _ => true,
    }
}

pub fn spawn_tutorial_coach_overlay(commands: &mut Commands) {
    commands
        .spawn((
            TutorialCoachCardRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(362.0),
                top: Val::Px(44.0),
                width: Val::Px(460.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(11.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.09, 0.16, 0.94)),
            BorderColor(Color::srgb(0.98, 0.84, 0.32)),
            ZIndex(26),
        ))
        .with_children(|root| {
            root.spawn((
                TutorialCoachCardBody,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
            ));
        });

    commands
        .spawn((
            TutorialSidebarArrowBanner,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(276.0),
                top: Val::Px(250.0),
                display: Display::None,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.36, 0.24, 0.04, 0.96)),
            BorderColor(Color::srgb(1.0, 0.88, 0.28)),
            ZIndex(28),
            PickingBehavior::IGNORE,
        ))
        .with_children(|arrow| {
            arrow.spawn((
                Text::new("CLICK CAMEO IN SIDEBAR >>>"),
                TextFont {
                    font_size: 11.5,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.96, 0.62)),
            ));
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_tutorial_coach(
    mut commands: Commands,
    time: Res<Time>,
    mod_res: Res<ModResource>,
    sim_res: Res<SimWorldResource>,
    mut interaction: ResMut<ClientInteractionState>,
    mut ui_blocker: ResMut<UiInputBlocker>,
    mut coach: ResMut<TutorialCoachState>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<&RtsCameraRig, Without<PreviewStageCamera>>,
    mut root_q: Query<
        (&mut Node, &ComputedNode, &GlobalTransform),
        (
            With<TutorialCoachCardRoot>,
            Without<TutorialSidebarArrowBanner>,
        ),
    >,
    body_q: Query<Entity, With<TutorialCoachCardBody>>,
    mut arrow_q: Query<
        (&mut Node, &mut BorderColor),
        (
            With<TutorialSidebarArrowBanner>,
            Without<TutorialCoachCardRoot>,
        ),
    >,
) {
    let is_bootcamp_mission = interaction.active_mission_id.as_deref() == Some(BOOTCAMP_MISSION_ID)
        || interaction.active_map_id == BOOTCAMP_MISSION_ID;
    if is_bootcamp_mission
        && !coach.active
        && !coach.completed
        && coach.dismissed_for_mission.as_deref() != Some(BOOTCAMP_MISSION_ID)
    {
        coach.reset_and_activate();
    } else if !is_bootcamp_mission && coach.active {
        coach.active = false;
    }

    let show = coach.active
        && interaction.app_phase == AppPhase::InGame
        && !interaction.menu_open
        && !interaction.studio_open;

    if let Ok((mut root_node, computed, gt)) = root_q.get_single_mut() {
        root_node.display = if show { Display::Flex } else { Display::None };
        if show {
            if let Ok(window) = window_q.get_single() {
                if let Some(cursor) = window.cursor_position() {
                    let rect = ui_node_logical_rect(computed, gt, window.scale_factor());
                    if rect.width() > 2.0 && rect.height() > 2.0 && rect.contains(cursor) {
                        ui_blocker.over_player_hud = true;
                    }
                }
            }
        }
    }

    let show_sidebar_arrow = show
        && !coach.completed
        && (matches!(coach.step_index, 3 | 4)
            || (coach.step_index == 5
                && interaction.active_sidebar_tab == ProductionSidebarTab::Structures));
    for (mut arrow_node, mut arrow_border) in &mut arrow_q {
        arrow_node.display = if show_sidebar_arrow {
            Display::Flex
        } else {
            Display::None
        };
        if show_sidebar_arrow {
            let pulse = (time.elapsed_secs() * 5.5).sin() * 0.5 + 0.5;
            arrow_node.right = Val::Px(274.0 + pulse * 10.0);
            *arrow_border = BorderColor(Color::srgb(1.0, 0.72 + 0.26 * pulse, 0.22));
        }
    }

    if !show {
        return;
    }

    let mut cam_moved = false;
    if let Ok(rig) = camera_q.get_single() {
        let base_focus = *coach.baseline_cam_focus.get_or_insert(rig.focus);
        let base_dist = *coach.baseline_cam_distance.get_or_insert(rig.distance);
        cam_moved =
            rig.focus.distance(base_focus) >= 3.5 || (rig.distance - base_dist).abs() >= 1.5;
    }

    if !coach.completed
        && is_tutorial_step_satisfied(
            coach.step_index,
            &sim_res.0,
            &mod_res.0,
            &interaction,
            cam_moved,
        )
    {
        if coach.step_index + 1 < TUTORIAL_STEP_COUNT {
            coach.step_index += 1;
            let (next_title, _) =
                tutorial_step_title_and_hint(&interaction.language, coach.step_index);
            interaction.push_eva_alert(format!("BOOTCAMP: {next_title}"));
        } else {
            coach.completed = true;
            interaction.push_eva_alert(
                "EVA: Bootcamp Basic Training Complete — Destroy the hostile outpost to claim victory!",
            );
        }
    }

    let sig = format!(
        "step:{}:done:{}:lang:{}",
        coach.step_index, coach.completed, interaction.language
    );
    if coach.last_rendered_sig == sig {
        return;
    }
    coach.last_rendered_sig = sig;

    let Ok(body_ent) = body_q.get_single() else {
        return;
    };
    commands.entity(body_ent).despawn_descendants();

    let lang = interaction.language.clone();
    let step = coach.step_index;
    let completed = coach.completed;

    commands.entity(body_ent).with_children(|col| {
        if completed {
            col.spawn((
                Text::new(sanitize_ui_text(if lang == "de" {
                    "BOOTCAMP-TRAINING ABGESCHLOSSEN!"
                } else {
                    "BOOTCAMP TACTICAL TRAINING COMPLETE!"
                })),
                TextFont {
                    font_size: 12.5,
                    ..default()
                },
                TextColor(Color::srgb(0.42, 0.98, 0.58)),
            ));
            col.spawn((
                Text::new(sanitize_ui_text(if lang == "de" {
                    "Alle 8 Grundlagenschritte gemeistert! Vernichte nun den feindlichen Außenposten."
                } else {
                    "All 8 tactical fundamentals mastered! Destroy the enemy outpost to complete the operation."
                })),
                TextFont {
                    font_size: 10.5,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.94, 1.0)),
            ));
            spawn_styled_button(
                col,
                if lang == "de" { "COACH SCHLIESSEN" } else { "CLOSE COACH" },
                SidebarAction::DismissTutorialCoach,
                Color::srgb(0.20, 0.42, 0.28),
                Val::Auto,
            );
        } else {
            let (title, hint) = tutorial_step_title_and_hint(&lang, step);
            col.spawn((
                Text::new(sanitize_ui_text(title)),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.98, 0.88, 0.38)),
            ));
            col.spawn((
                Text::new(sanitize_ui_text(hint)),
                TextFont {
                    font_size: 10.5,
                    ..default()
                },
                TextColor(Color::srgb(0.90, 0.95, 1.0)),
            ));

            // Progress pips 1..8
            let mut pips = String::new();
            for idx in 0..TUTORIAL_STEP_COUNT {
                if idx < step {
                    pips.push_str("[OK] ");
                } else if idx == step {
                    pips.push_str("[>>] ");
                } else {
                    pips.push_str("[..] ");
                }
            }
            col.spawn((
                Text::new(pips),
                TextFont {
                    font_size: 9.5,
                    ..default()
                },
                TextColor(Color::srgb(0.56, 0.86, 0.98)),
            ));

            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|btns| {
                spawn_styled_button(
                    btns,
                    if lang == "de" {
                        "SCHRITT ÜBERSPRINGEN"
                    } else {
                        "SKIP STEP >>"
                    },
                    SidebarAction::AdvanceTutorialStep,
                    Color::srgb(0.22, 0.34, 0.48),
                    Val::Auto,
                );
                spawn_styled_button(
                    btns,
                    if lang == "de" { "AUSBLENDEN" } else { "DISMISS" },
                    SidebarAction::DismissTutorialCoach,
                    Color::srgb(0.38, 0.18, 0.20),
                    Val::Auto,
                );
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootcamp_step_titles_cover_all_eight_steps_in_en_and_de() {
        for step in 0..TUTORIAL_STEP_COUNT {
            let (en_t, en_h) = tutorial_step_title_and_hint("en", step);
            let (de_t, de_h) = tutorial_step_title_and_hint("de", step);
            assert!(!en_t.is_empty() && !en_h.is_empty());
            assert!(!de_t.is_empty() && !de_h.is_empty());
            assert_ne!(en_t, de_t);
        }
    }
}
