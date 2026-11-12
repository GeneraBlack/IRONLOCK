use bevy_egui::egui;
use rts_sdk::schema::{
    ProjectileStyle, VfxBlendMode, VfxDecalLinkSpec, VfxEffectDef, VfxEmitterDef, VfxEmitterShape,
};

pub(super) fn show_vfx_effect_and_emitters_inspector(
    ui: &mut egui::Ui,
    eff: &mut VfxEffectDef,
    selected_emitter_idx: &mut usize,
    delete_effect: &mut bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("Effect '{}':", eff.id));
        ui.add(
            egui::TextEdit::singleline(&mut eff.name)
                .desired_width(165.0)
                .hint_text("Display Name"),
        );
        ui.add(
            egui::DragValue::new(&mut eff.duration_secs)
                .range(0.05..=30.0)
                .speed(0.05)
                .prefix("Dur(s): "),
        );
        ui.checkbox(&mut eff.looping, "Looping");
        egui::ComboBox::from_id_salt("vfx_fallback_style_cb")
            .selected_text(format!("Fallback: {:?}", eff.fallback_style))
            .width(135.0)
            .show_ui(ui, |ui| {
                for st in [
                    ProjectileStyle::Bullet,
                    ProjectileStyle::Laser,
                    ProjectileStyle::Tesla,
                    ProjectileStyle::Radiation,
                    ProjectileStyle::Sonic,
                    ProjectileStyle::MindControl,
                    ProjectileStyle::Missile,
                ] {
                    ui.selectable_value(&mut eff.fallback_style, st, format!("{st:?}"));
                }
            });
        if ui.small_button("🗑 Delete Effect").clicked() {
            *delete_effect = true;
        }
    });

    // Camera Shake & Ground Decal Link
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::DragValue::new(&mut eff.camera_shake_intensity)
                .range(0.0..=4.0)
                .speed(0.05)
                .prefix("CamShake: "),
        );
        ui.add(
            egui::DragValue::new(&mut eff.camera_shake_radius)
                .range(0.0..=64.0)
                .speed(0.5)
                .prefix("ShakeRad: "),
        );

        let mut has_decal = eff.ground_decal.is_some();
        if ui.checkbox(&mut has_decal, "Linked Ground Decal").changed() {
            eff.ground_decal = has_decal.then(VfxDecalLinkSpec::default);
        }
        if let Some(dec) = eff.ground_decal.as_mut() {
            egui::ComboBox::from_id_salt("vfx_decal_kind_cb")
                .selected_text(&dec.decal_kind)
                .width(115.0)
                .show_ui(ui, |ui| {
                    for dk in [
                        "scorch_blast",
                        "crater_shell",
                        "radiation_pool",
                        "frost_crack",
                        "plasma_burn",
                    ] {
                        ui.selectable_value(&mut dec.decal_kind, dk.to_string(), dk);
                    }
                });
            ui.add(
                egui::DragValue::new(&mut dec.radius)
                    .range(0.2..=18.0)
                    .speed(0.1)
                    .prefix("Rad: "),
            );
            ui.add(
                egui::DragValue::new(&mut dec.fade_time_secs)
                    .range(1.0..=180.0)
                    .speed(0.5)
                    .prefix("Fade(s): "),
            );
        }
    });

    ui.separator();

    // Multi-Emitter Tab Strip
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("Emitters ({}):", eff.emitters.len()));
        for (ei, em) in eff.emitters.iter().enumerate() {
            let active = *selected_emitter_idx == ei;
            if ui
                .selectable_label(active, format!("#{} {} ({:?})", ei + 1, em.id, em.shape))
                .clicked()
            {
                *selected_emitter_idx = ei;
            }
        }
        if ui.small_button("+ Add Emitter").clicked() {
            let n = eff.emitters.len() + 1;
            eff.emitters.push(VfxEmitterDef {
                id: format!("emitter_{n}"),
                ..Default::default()
            });
            *selected_emitter_idx = eff.emitters.len() - 1;
        }
    });

    if eff.emitters.is_empty() {
        ui.weak("Click [+ Add Emitter] to add a particle/beam/ribbon emitter.");
        return;
    }

    *selected_emitter_idx = (*selected_emitter_idx).min(eff.emitters.len() - 1);
    let idx = *selected_emitter_idx;
    let can_remove = eff.emitters.len() > 1;
    let mut remove_em = false;

    if let Some(em) = eff.emitters.get_mut(idx) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Emitter ID:");
                ui.add(egui::TextEdit::singleline(&mut em.id).desired_width(115.0));
                ui.label("Shape:");
                egui::ComboBox::from_id_salt(("em_shape", idx))
                    .selected_text(format!("{:?}", em.shape))
                    .width(95.0)
                    .show_ui(ui, |ui| {
                        for sh in [
                            VfxEmitterShape::Point,
                            VfxEmitterShape::Sphere,
                            VfxEmitterShape::Cone,
                            VfxEmitterShape::Ring,
                            VfxEmitterShape::Beam,
                            VfxEmitterShape::Ribbon,
                        ] {
                            ui.selectable_value(&mut em.shape, sh, format!("{sh:?}"));
                        }
                    });
                ui.label("Blend:");
                egui::ComboBox::from_id_salt(("em_blend", idx))
                    .selected_text(format!("{:?}", em.blend_mode))
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut em.blend_mode,
                            VfxBlendMode::Additive,
                            "Additive (HDR)",
                        );
                        ui.selectable_value(
                            &mut em.blend_mode,
                            VfxBlendMode::AlphaBlend,
                            "AlphaBlend (Smoke)",
                        );
                    });
                if can_remove && ui.small_button("✖ Remove Emitter").clicked() {
                    remove_em = true;
                }
            });

            // Emission Rate / Burst / Shape Geometry / Physics
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut em.burst_count)
                        .range(0..=512)
                        .prefix("Burst: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.continuous_rate_per_sec)
                        .range(0.0..=500.0)
                        .speed(1.0)
                        .prefix("Rate/s: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.cone_angle_deg)
                        .range(0.0..=180.0)
                        .speed(1.0)
                        .prefix("Cone°: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.sphere_radius)
                        .range(0.0..=12.0)
                        .speed(0.05)
                        .prefix("SphereRad: "),
                );
                ui.checkbox(&mut em.looping, "Loop");
            });

            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut em.lifetime_range_secs[0])
                        .range(0.05..=15.0)
                        .speed(0.05)
                        .prefix("LifeMin: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.lifetime_range_secs[1])
                        .range(0.05..=15.0)
                        .speed(0.05)
                        .prefix("LifeMax: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.speed_range[0])
                        .range(0.0..=80.0)
                        .speed(0.2)
                        .prefix("SpdMin: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.speed_range[1])
                        .range(0.0..=80.0)
                        .speed(0.2)
                        .prefix("SpdMax: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.gravity)
                        .range(-40.0..=40.0)
                        .speed(0.25)
                        .prefix("Grav: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.drag)
                        .range(0.0..=20.0)
                        .speed(0.1)
                        .prefix("Drag: "),
                );
            });

            // Beam / Ribbon Width + Local Offset + Optional Sprite Texture
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::DragValue::new(&mut em.beam_width)
                        .range(0.02..=6.0)
                        .speed(0.02)
                        .prefix("BeamW: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.ribbon_width)
                        .range(0.02..=6.0)
                        .speed(0.02)
                        .prefix("RibbonW: "),
                );
                ui.add(
                    egui::DragValue::new(&mut em.local_offset[1])
                        .range(-5.0..=10.0)
                        .speed(0.05)
                        .prefix("OffsetY: "),
                );
                let mut tex_buf = em.texture_path.clone().unwrap_or_default();
                ui.label("Sprite:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut tex_buf)
                            .desired_width(145.0)
                            .hint_text("textures/vfx/spark.png"),
                    )
                    .changed()
                {
                    em.texture_path =
                        (!tex_buf.trim().is_empty()).then(|| tex_buf.trim().to_string());
                }
            });

            // 3-Stop Size Ramp
            ui.horizontal_wrapped(|ui| {
                ui.strong("3-Stop Size Ramp:");
                ui.add(
                    egui::Slider::new(&mut em.size_ramp[0], 0.01..=5.0)
                        .text("Start")
                        .fixed_decimals(2),
                );
                ui.add(
                    egui::Slider::new(&mut em.size_ramp[1], 0.01..=5.0)
                        .text("Mid")
                        .fixed_decimals(2),
                );
                ui.add(
                    egui::Slider::new(&mut em.size_ramp[2], 0.01..=5.0)
                        .text("End")
                        .fixed_decimals(2),
                );
            });

            // 3-Stop HDR Color/Alpha Ramp + Painted Visual Gradient Bar
            ui.strong("3-Stop HDR Color / Alpha Ramp (`Start -> Mid -> End`):");
            draw_hdr_color_ramp_preview_bar(ui, &em.color_ramp_rgba);
            for (stop_idx, stop_lbl) in [
                (0usize, "Start (t=0.0)"),
                (1, "Mid (t=0.5)"),
                (2, "End (t=1.0)"),
            ] {
                let c = &mut em.color_ramp_rgba[stop_idx];
                let peak = c[0].max(c[1]).max(c[2]).max(1.0);
                let mut base_rgb = [
                    (c[0] / peak).clamp(0.0, 1.0),
                    (c[1] / peak).clamp(0.0, 1.0),
                    (c[2] / peak).clamp(0.0, 1.0),
                ];
                let mut hdr_gain = peak;
                ui.horizontal_wrapped(|ui| {
                    ui.monospace(stop_lbl);
                    let rgb_changed = ui.color_edit_button_rgb(&mut base_rgb).changed();
                    let gain_changed = ui
                        .add(
                            egui::Slider::new(&mut hdr_gain, 0.2..=6.0)
                                .text("HDRx")
                                .fixed_decimals(2),
                        )
                        .changed();
                    if rgb_changed || gain_changed {
                        c[0] = base_rgb[0] * hdr_gain;
                        c[1] = base_rgb[1] * hdr_gain;
                        c[2] = base_rgb[2] * hdr_gain;
                    }
                    ui.add(
                        egui::Slider::new(&mut c[3], 0.0..=1.0)
                            .text("Alpha")
                            .fixed_decimals(2),
                    );
                });
            }
        });
    }

    if remove_em {
        eff.emitters.remove(idx);
        *selected_emitter_idx = 0;
    }
}

fn draw_hdr_color_ramp_preview_bar(ui: &mut egui::Ui, ramp: &[[f32; 4]; 3]) {
    let w = ui.available_width().clamp(160.0, 440.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let segs = 28usize;
    let seg_w = rect.width() / segs as f32;
    for i in 0..segs {
        let t = (i as f32 + 0.5) / segs as f32;
        let rgba = sample_three_stop_ramp(ramp, t);
        let col = egui::Color32::from_rgba_unmultiplied(
            (rgba[0].clamp(0.0, 1.0) * 255.0) as u8,
            (rgba[1].clamp(0.0, 1.0) * 255.0) as u8,
            (rgba[2].clamp(0.0, 1.0) * 255.0) as u8,
            (rgba[3].clamp(0.08, 1.0) * 255.0) as u8,
        );
        let sr = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + i as f32 * seg_w, rect.min.y),
            egui::pos2(rect.min.x + (i + 1) as f32 * seg_w, rect.max.y),
        );
        painter.rect_filled(sr, 0.0, col);
    }
    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(95, 118, 148)),
        egui::StrokeKind::Inside,
    );
}

fn sample_three_stop_ramp(ramp: &[[f32; 4]; 3], t: f32) -> [f32; 4] {
    let (a, b, u) = if t <= 0.5 {
        (ramp[0], ramp[1], (t * 2.0).clamp(0.0, 1.0))
    } else {
        (ramp[1], ramp[2], ((t - 0.5) * 2.0).clamp(0.0, 1.0))
    };
    [
        a[0] + (b[0] - a[0]) * u,
        a[1] + (b[1] - a[1]) * u,
        a[2] + (b[2] - a[2]) * u,
        a[3] + (b[3] - a[3]) * u,
    ]
}
