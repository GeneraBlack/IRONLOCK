use super::StudioState;
use bevy_egui::egui;
use rts_asset_pipeline::cook::{cook_standalone_game, CookStandaloneOptions, CookStandaloneReport};
use rts_asset_pipeline::packager::pack_mod_to_file;
use rts_network::lobby_registry::{
    LobbyRegistryClient, WorkshopItemKind, WorkshopItemMetadata, WorkshopPublishRequest,
    DEFAULT_CENTRAL_LOBBY_URL,
};
use rts_network::player_identity::{PlayerIdentityProfile, DEFAULT_PLAYER_IDENTITY_PATH};
use rts_sdk::loader::LoadedMod;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};

pub enum ExportWizardMsg {
    Log(String),
    Finished(Result<CookStandaloneReport, String>),
    WorkshopPublished(Result<WorkshopItemMetadata, String>),
}

pub struct ExportWizardState {
    pub open: bool,
    pub game_title: String,
    pub output_dir: String,
    pub lock_player_mode: bool,
    pub running: bool,
    pub progress_log: Vec<String>,
    pub last_report: Option<CookStandaloneReport>,
    pub receiver: Option<Mutex<mpsc::Receiver<ExportWizardMsg>>>,

    // Community Workshop Publisher State (Task 225-Studio)
    pub workshop_registry_url: String,
    pub workshop_item_id: String,
    pub workshop_title: String,
    pub workshop_description: String,
    pub workshop_tags_csv: String,
    pub workshop_kind: WorkshopItemKind,
    pub last_workshop_metadata: Option<WorkshopItemMetadata>,
}

impl Default for ExportWizardState {
    fn default() -> Self {
        Self {
            open: false,
            game_title: String::new(),
            output_dir: String::new(),
            lock_player_mode: true,
            running: false,
            progress_log: Vec::new(),
            last_report: None,
            receiver: None,
            workshop_registry_url: DEFAULT_CENTRAL_LOBBY_URL.to_string(),
            workshop_item_id: String::new(),
            workshop_title: String::new(),
            workshop_description: String::new(),
            workshop_tags_csv: "RTS,Mod".to_string(),
            workshop_kind: WorkshopItemKind::Mod,
            last_workshop_metadata: None,
        }
    }
}

impl ExportWizardState {
    pub fn open_for_mod(&mut self, loaded: &LoadedMod, workspace_root: &Path) {
        self.open = true;
        if self.game_title.trim().is_empty() {
            self.game_title = loaded.manifest.name.clone();
        }
        if self.output_dir.trim().is_empty() {
            let default_out = workspace_root
                .join("dist")
                .join(format!("{}-standalone", loaded.manifest.id));
            self.output_dir = default_out.display().to_string();
        }
        if self.workshop_item_id.trim().is_empty() {
            self.workshop_item_id = loaded.manifest.id.clone();
        }
        if self.workshop_title.trim().is_empty() {
            self.workshop_title = loaded.manifest.name.clone();
        }
        if self.workshop_description.trim().is_empty() {
            self.workshop_description = format!(
                "{} v{} created in IRONLOCK Creator Workbench.",
                loaded.manifest.name, loaded.manifest.version
            );
        }
    }

    pub fn poll_messages(&mut self) -> Option<String> {
        let mut banner_update = None;
        let mut finished_channel = false;
        if let Some(ref rx_mtx) = self.receiver {
            if let Ok(rx) = rx_mtx.lock() {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        ExportWizardMsg::Log(line) => {
                            self.progress_log.push(line);
                        }
                        ExportWizardMsg::Finished(res) => {
                            self.running = false;
                            finished_channel = true;
                            match res {
                                Ok(rep) => {
                                    let summary = format!(
                                        "✔ Standalone build '{}' ready -> {} ({} files, {:.1} KB RTSMOD03 archive, hash 0x{:016x})",
                                        rep.game_title,
                                        rep.archive_path
                                            .parent()
                                            .and_then(|p| p.parent())
                                            .unwrap_or(&rep.archive_path)
                                            .display(),
                                        rep.packed_files_count,
                                        rep.archive_bytes as f64 / 1024.0,
                                        rep.content_hash
                                    );
                                    self.progress_log.push(summary.clone());
                                    banner_update = Some(summary);
                                    self.last_report = Some(rep);
                                }
                                Err(err) => {
                                    let msg = format!("✖ Standalone export failed: {err}");
                                    self.progress_log.push(msg.clone());
                                    banner_update = Some(msg);
                                }
                            }
                        }
                        ExportWizardMsg::WorkshopPublished(res) => {
                            self.running = false;
                            finished_channel = true;
                            match res {
                                Ok(meta) => {
                                    let summary = format!(
                                        "✔ Published '{}' ({} v{}) to Workshop -> id '{}' ({:.1} KB, sha256 {}..)",
                                        meta.title,
                                        meta.kind.as_str(),
                                        meta.version,
                                        meta.id,
                                        meta.size_bytes as f64 / 1024.0,
                                        &meta.sha256[..meta.sha256.len().min(12)]
                                    );
                                    self.progress_log.push(summary.clone());
                                    banner_update = Some(summary);
                                    self.last_workshop_metadata = Some(meta);
                                }
                                Err(err) => {
                                    let msg = format!("✖ Workshop publish failed: {err}");
                                    self.progress_log.push(msg.clone());
                                    banner_update = Some(msg);
                                }
                            }
                        }
                    }
                }
            }
        }
        if finished_channel {
            self.receiver = None;
        }
        banner_update
    }

    pub fn start_background_cook(&mut self, mod_path: PathBuf) {
        if self.running {
            return;
        }
        let title_opt =
            (!self.game_title.trim().is_empty()).then(|| self.game_title.trim().to_string());
        let output_dir = PathBuf::from(self.output_dir.trim());
        let lock_player_mode = self.lock_player_mode;

        let (tx, rx) = mpsc::channel();
        self.running = true;
        self.last_report = None;
        self.progress_log.clear();
        self.progress_log.push(format!(
            "[1/4] Starting standalone cook for '{}' -> '{}'...",
            mod_path.display(),
            output_dir.display()
        ));
        self.receiver = Some(Mutex::new(rx));

        std::thread::spawn(move || {
            let _ = tx.send(ExportWizardMsg::Log(
                "[2/4] Validating mod schemas, scripts, assets & map playability...".to_string(),
            ));
            let opts = CookStandaloneOptions {
                mod_path,
                output_dir,
                title: title_opt,
                lock_player_mode,
            };
            let _ = tx.send(ExportWizardMsg::Log(
                "[3/4] Packing Zlib RTSMOD03 VFS archive, writing game_bundle.toml & Play launchers..."
                    .to_string(),
            ));
            let res = cook_standalone_game(&opts).map_err(|e| format!("{e:#}"));
            if res.is_ok() {
                let _ = tx.send(ExportWizardMsg::Log(
                    "[4/4] Zero-extraction VFS roundtrip verification passed!".to_string(),
                ));
            }
            let _ = tx.send(ExportWizardMsg::Finished(res));
        });
    }

    pub fn start_background_workshop_publish(&mut self, loaded: &LoadedMod, workspace_root: &Path) {
        if self.running {
            return;
        }
        let registry_url = self.workshop_registry_url.trim().to_string();
        let item_id = self.workshop_item_id.trim().to_string();
        let title = self.workshop_title.trim().to_string();
        let description = self.workshop_description.trim().to_string();
        let kind = self.workshop_kind;
        let version = loaded.manifest.version.clone();
        let tags: Vec<String> = self
            .workshop_tags_csv
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let mod_root = loaded.root_dir.clone();
        let entry_map = loaded.entry_map.clone();
        let identity_path = workspace_root.join(DEFAULT_PLAYER_IDENTITY_PATH);

        let (tx, rx) = mpsc::channel();
        self.running = true;
        self.last_workshop_metadata = None;
        self.progress_log.clear();
        self.progress_log.push(format!(
            "[1/3] Preparing {} Workshop upload '{}' -> {}...",
            kind.as_str(),
            item_id,
            registry_url
        ));
        self.receiver = Some(Mutex::new(rx));

        std::thread::spawn(move || {
            let outcome: Result<WorkshopItemMetadata, String> = (|| {
                let profile = PlayerIdentityProfile::load_or_create(&identity_path, "Creator")
                    .map_err(|e| format!("Identity profile error: {e:#}"))?;
                let (payload_bytes, content_hash_hex) = match kind {
                    WorkshopItemKind::Mod => {
                        let _ = tx.send(ExportWizardMsg::Log(
                            "[2/3] Validating & packing RTSMOD03 .rtsmod archive...".to_string(),
                        ));
                        let tmp_pkg = std::env::temp_dir().join(format!(
                            "ironlock_workshop_pub_{}_{}.rtsmod",
                            item_id,
                            std::process::id()
                        ));
                        let pkg =
                            pack_mod_to_file(&mod_root, &tmp_pkg).map_err(|e| format!("{e:#}"))?;
                        let bytes = std::fs::read(&tmp_pkg)
                            .map_err(|e| format!("Read packed .rtsmod failed: {e}"))?;
                        let _ = std::fs::remove_file(&tmp_pkg);
                        (bytes, format!("{:016x}", pkg.content_hash))
                    }
                    WorkshopItemKind::Map => {
                        let _ = tx.send(ExportWizardMsg::Log(
                            "[2/3] Serializing active MapDef .toml payload...".to_string(),
                        ));
                        let tmp_map = std::env::temp_dir().join(format!(
                            "ironlock_workshop_map_{}_{}.toml",
                            entry_map.id,
                            std::process::id()
                        ));
                        entry_map
                            .save_to_file(&tmp_map)
                            .map_err(|e| format!("Serialize map TOML failed: {e:#}"))?;
                        let bytes = std::fs::read(&tmp_map)
                            .map_err(|e| format!("Read map TOML failed: {e}"))?;
                        let _ = std::fs::remove_file(&tmp_map);
                        (bytes, String::new())
                    }
                };

                let _ = tx.send(ExportWizardMsg::Log(format!(
                    "[3/3] Signing & uploading {:.1} KB payload as '{}' ({})...",
                    payload_bytes.len() as f64 / 1024.0,
                    profile.display_name,
                    profile.player_id
                )));

                let req = WorkshopPublishRequest {
                    id: item_id,
                    title,
                    description,
                    kind,
                    version,
                    tags,
                    mod_content_hash: content_hash_hex,
                };
                LobbyRegistryClient::publish_workshop_item(
                    &registry_url,
                    &profile,
                    &req,
                    &payload_bytes,
                )
                .map_err(|e| format!("{e:#}"))
            })();
            let _ = tx.send(ExportWizardMsg::WorkshopPublished(outcome));
        });
    }
}

pub fn show_export_standalone_wizard_modal(
    ctx: &egui::Context,
    state: &mut StudioState,
    loaded: &LoadedMod,
    workspace_root: &Path,
) {
    if let Some(banner) = state.export_wizard.poll_messages() {
        state.status_banner = banner;
    }
    if state.export_wizard.running {
        ctx.request_repaint();
    }
    if !state.export_wizard.open {
        return;
    }

    let mut open = state.export_wizard.open;
    egui::Window::new("📦 Export Standalone Game & Publish to Workshop")
        .open(&mut open)
        .resizable(true)
        .default_width(610.0)
        .show(ctx, |ui| {
            ui.label(
                "Cook a self-contained standalone RTS distribution (RTSMOD03 VFS archive + launchers) or publish a signed .rtsmod / .toml package directly to the Community Workshop.",
            );
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Source Mod:");
                ui.strong(format!(
                    "{} ({} v{}) — {}",
                    loaded.manifest.name,
                    loaded.manifest.id,
                    loaded.manifest.version,
                    loaded.root_dir.display()
                ));
            });

            // 1. Standalone Game Distribution Cook
            egui::CollapsingHeader::new("1. Standalone Game Distribution Cook")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Standalone Window Title:");
                        ui.text_edit_singleline(&mut state.export_wizard.game_title);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Output Directory:");
                        ui.text_edit_singleline(&mut state.export_wizard.output_dir);
                        if ui.small_button("Reset Default").clicked() {
                            state.export_wizard.output_dir = workspace_root
                                .join("dist")
                                .join(format!("{}-standalone", loaded.manifest.id))
                                .display()
                                .to_string();
                        }
                    });

                    ui.checkbox(
                        &mut state.export_wizard.lock_player_mode,
                        "Lock Standalone Player Mode (`lock_player_mode = true`: disables [F12] Studio & --studio flag)",
                    );

                    ui.horizontal(|ui| {
                        let can_run = !state.export_wizard.running
                            && !state.export_wizard.output_dir.trim().is_empty();
                        if ui
                            .add_enabled(
                                can_run,
                                egui::Button::new(
                                    egui::RichText::new("🚀 Cook & Package Standalone Game")
                                        .color(egui::Color32::from_rgb(110, 240, 155)),
                                ),
                            )
                            .clicked()
                        {
                            state
                                .export_wizard
                                .start_background_cook(loaded.root_dir.clone());
                        }
                    });

                    if let Some(ref rep) = state.export_wizard.last_report {
                        ui.colored_label(
                            egui::Color32::from_rgb(105, 235, 155),
                            format!(
                                "Bundle Verified: {} | {} packed files ({:.1} KB -> {:.1} KB compressed) | Copied {} binary(s)",
                                rep.bundle_config_path.display(),
                                rep.packed_files_count,
                                rep.uncompressed_bytes as f64 / 1024.0,
                                rep.archive_bytes as f64 / 1024.0,
                                rep.copied_binaries.len()
                            ),
                        );
                    }
                });

            ui.separator();

            // 2. Community Workshop Publisher
            egui::CollapsingHeader::new("2. Publish to Community Workshop (Signed Upload)")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Workshop Server URL:");
                        ui.text_edit_singleline(&mut state.export_wizard.workshop_registry_url);
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Package Kind:");
                        ui.selectable_value(
                            &mut state.export_wizard.workshop_kind,
                            WorkshopItemKind::Mod,
                            "Mod (.rtsmod)",
                        );
                        ui.selectable_value(
                            &mut state.export_wizard.workshop_kind,
                            WorkshopItemKind::Map,
                            "Skirmish Map (.toml)",
                        );
                        ui.label("Workshop ID:");
                        ui.text_edit_singleline(&mut state.export_wizard.workshop_item_id);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Listing Title:");
                        ui.text_edit_singleline(&mut state.export_wizard.workshop_title);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Tags (CSV):");
                        ui.text_edit_singleline(&mut state.export_wizard.workshop_tags_csv);
                    });
                    ui.label("Description:");
                    ui.text_edit_multiline(&mut state.export_wizard.workshop_description);

                    let can_pub = !state.export_wizard.running
                        && !state.export_wizard.workshop_registry_url.trim().is_empty()
                        && !state.export_wizard.workshop_item_id.trim().is_empty()
                        && !state.export_wizard.workshop_title.trim().is_empty();
                    if ui
                        .add_enabled(
                            can_pub,
                            egui::Button::new(
                                egui::RichText::new("☁ Publish Package to Community Workshop")
                                    .color(egui::Color32::from_rgb(125, 205, 255)),
                            ),
                        )
                        .clicked()
                    {
                        state
                            .export_wizard
                            .start_background_workshop_publish(loaded, workspace_root);
                    }

                    if let Some(ref meta) = state.export_wizard.last_workshop_metadata {
                        ui.colored_label(
                            egui::Color32::from_rgb(120, 225, 255),
                            format!(
                                "✔ Workshop Live: '{}' (id: {}, v{}, {} bytes, hash: {})",
                                meta.title,
                                meta.id,
                                meta.version,
                                meta.size_bytes,
                                if meta.mod_content_hash.is_empty() {
                                    meta.sha256.as_str()
                                } else {
                                    meta.mod_content_hash.as_str()
                                }
                            ),
                        );
                    }
                });

            if state.export_wizard.running {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.colored_label(
                        egui::Color32::from_rgb(245, 205, 95),
                        "Working on background thread...",
                    );
                });
            }

            ui.separator();
            ui.strong("Live Cook, Validation & Publish Log:");
            egui::ScrollArea::vertical()
                .id_salt("export_standalone_log_scroll")
                .max_height(160.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if state.export_wizard.progress_log.is_empty() {
                        ui.weak("Ready. Choose Standalone Cook or Community Workshop Publish above.");
                    }
                    for line in &state.export_wizard.progress_log {
                        ui.monospace(line);
                    }
                });
        });

    if !state.export_wizard.running {
        state.export_wizard.open = open;
    }
}
