use crate::modals::{
    render_edit_profile_modal, render_fingerprint_modal, render_password_modal, FingerprintAction,
    FingerprintPrompt, ModalState, PasswordAction, ProfileEditAction,
};
use crate::profiles::{discover_ssh_keys, ConnectionProfile, ProfileStore};
use crate::session::{PromptType, SshSession};
use crate::sftp::SftpBrowser;
use crate::terminal::render_terminal_ui;
use crate::theme::{build_dark_visuals, build_light_visuals, AppSettings, ThemeMode};
use crate::toolbox::{CustomCmdDraft, ToolboxCategory, ToolboxStore};
use crate::tunnels::{PortTunnel, TunnelManager};
use eframe::egui::{self, Color32, FontFamily, FontId, Key, Modifiers, RichText, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Terminal,
    Sftp,
    Tunnels,
    Settings,
}

pub struct FerroSshApp {
    profile_store: ProfileStore,
    sessions: Vec<SshSession>,
    active_session_idx: usize,
    search_query: String,
    quick_connect: String,
    tunnel_manager: TunnelManager,
    sftp_browser: Option<SftpBrowser>,
    modal_state: ModalState,
    view_mode: ViewMode,
    available_keys: Vec<std::path::PathBuf>,
    settings: AppSettings,
    last_applied_dark: Option<bool>,
    status_msg: Option<(String, std::time::Instant)>,
    toolbox: ToolboxStore,
    show_toolbox: bool,
    toolbox_search: String,
    toolbox_category: ToolboxCategory,
    custom_cmd_draft: Option<CustomCmdDraft>,
}

impl FerroSshApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = AppSettings::load();
        let is_dark = settings.theme_mode.is_dark(cc.integration_info.system_theme);
        let visuals = if is_dark {
            build_dark_visuals()
        } else {
            build_light_visuals()
        };
        cc.egui_ctx.set_visuals(visuals);

        let profile_store = ProfileStore::load();
        let available_keys = discover_ssh_keys();
        let toolbox = ToolboxStore::load();

        let mut app = Self {
            profile_store,
            sessions: Vec::new(),
            active_session_idx: 0,
            search_query: String::new(),
            quick_connect: String::new(),
            tunnel_manager: TunnelManager::new(),
            sftp_browser: None,
            modal_state: ModalState::default(),
            view_mode: ViewMode::Terminal,
            available_keys,
            settings,
            last_applied_dark: Some(is_dark),
            status_msg: None,
            toolbox,
            show_toolbox: true,
            toolbox_search: String::new(),
            toolbox_category: ToolboxCategory::All,
            custom_cmd_draft: None,
        };

        // Parse command line arguments for direct tab selection and connection
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--view" | "-v" => {
                    if i + 1 < args.len() {
                        match args[i + 1].to_lowercase().as_str() {
                            "sftp" => app.view_mode = ViewMode::Sftp,
                            "tunnels" | "tunnel" => app.view_mode = ViewMode::Tunnels,
                            "settings" => app.view_mode = ViewMode::Settings,
                            _ => app.view_mode = ViewMode::Terminal,
                        }
                        i += 1;
                    }
                }
                "--theme" | "-t" => {
                    if i + 1 < args.len() {
                        match args[i + 1].to_lowercase().as_str() {
                            "light" => app.settings.theme_mode = ThemeMode::Light,
                            "dark" => app.settings.theme_mode = ThemeMode::Dark,
                            _ => app.settings.theme_mode = ThemeMode::System,
                        }
                        app.last_applied_dark = None;
                        i += 1;
                    }
                }
                "--connect" | "-c" => {
                    if i + 1 < args.len() {
                        let target = &args[i + 1];
                        if let Some(profile) = app
                            .profile_store
                            .profiles
                            .iter()
                            .find(|p| p.name.eq_ignore_ascii_case(target) || p.id == *target)
                            .cloned()
                        {
                            app.connect_to_profile(profile);
                        } else {
                            app.quick_connect = target.clone();
                            app.connect_quick();
                        }
                        i += 1;
                    }
                }
                arg if !arg.starts_with('-') => {
                    if let Some(profile) = app
                        .profile_store
                        .profiles
                        .iter()
                        .find(|p| p.name.eq_ignore_ascii_case(arg) || p.id == *arg)
                        .cloned()
                    {
                        app.connect_to_profile(profile);
                    } else {
                        app.quick_connect = arg.to_string();
                        app.connect_quick();
                    }
                }
                _ => {}
            }
            i += 1;
        }

        app
    }

    pub fn set_status(&mut self, msg: &str) {
        self.status_msg = Some((msg.to_string(), std::time::Instant::now()));
    }

    pub fn connect_to_profile(&mut self, profile: ConnectionProfile) {
        match SshSession::new(profile.clone(), 80, 24) {
            Ok(session) => {
                self.sessions.push(session);
                self.active_session_idx = self.sessions.len() - 1;
                self.view_mode = ViewMode::Terminal;
                self.set_status(&format!("Connected to {}", profile.name));
            }
            Err(e) => {
                self.set_status(&format!("Connection error: {}", e));
            }
        }
    }

    pub fn connect_quick(&mut self) {
        let input = self.quick_connect.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Parse user@host:port or host:port or user@host
        let (user, remainder) = if let Some((u, rest)) = input.split_once('@') {
            (u.to_string(), rest.to_string())
        } else {
            ("root".to_string(), input)
        };

        let (host, port) = if let Some((h, p)) = remainder.split_once(':') {
            (h.to_string(), p.parse().unwrap_or(22))
        } else {
            (remainder, 22)
        };

        let mut profile = ConnectionProfile::default();
        profile.name = format!("{}@{}", user, host);
        profile.username = user;
        profile.host = host;
        profile.port = port;

        self.connect_to_profile(profile);
    }
}

impl eframe::App for FerroSshApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Automatically sync theme with operating system setting or user override
        let system_theme = frame.info().system_theme;
        let should_be_dark = self.settings.theme_mode.is_dark(system_theme);
        if self.last_applied_dark != Some(should_be_dark) {
            let visuals = if should_be_dark {
                build_dark_visuals()
            } else {
                build_light_visuals()
            };
            ctx.set_visuals(visuals);
            self.last_applied_dark = Some(should_be_dark);
        }

        // Pump incoming PTY events for all active sessions
        for session in &mut self.sessions {
            session.update();
        }

        // Top Navigation Bar
        egui::TopBottomPanel::top("top_nav").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("⚡ FerroSSH").strong().size(16.0).color(Color32::from_rgb(52, 152, 219)));
                ui.separator();

                ui.selectable_value(&mut self.view_mode, ViewMode::Terminal, "💻 Terminal");
                ui.selectable_value(&mut self.view_mode, ViewMode::Sftp, "📁 SFTP Files");
                ui.selectable_value(&mut self.view_mode, ViewMode::Tunnels, "🔀 Port Tunnels");
                ui.selectable_value(&mut self.view_mode, ViewMode::Settings, "⚙ Settings");

                ui.separator();
                let theme_btn_label = match self.settings.theme_mode {
                    ThemeMode::System => {
                        let active_icon = if self.last_applied_dark.unwrap_or(true) { "🌙" } else { "☀" };
                        format!("🖥 Auto ({})", active_icon)
                    }
                    ThemeMode::Dark => "🌙 Dark".to_string(),
                    ThemeMode::Light => "☀ Light".to_string(),
                };
                if ui
                    .small_button(theme_btn_label)
                    .on_hover_text("Toggle theme preference: System (Auto) -> Dark -> Light")
                    .clicked()
                {
                    self.settings.theme_mode = match self.settings.theme_mode {
                        ThemeMode::System => ThemeMode::Dark,
                        ThemeMode::Dark => ThemeMode::Light,
                        ThemeMode::Light => ThemeMode::System,
                    };
                    self.settings.save();
                    self.last_applied_dark = None; // trigger instant redraw on next frame
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚡ Connect").clicked() {
                        self.connect_quick();
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.quick_connect)
                            .hint_text("Quick Connect: user@host[:port]")
                            .desired_width(220.0),
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        self.connect_quick();
                    }
                });
            });
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("bottom_status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let is_dark = ui.visuals().dark_mode;
                if let Some((msg, created)) = &self.status_msg {
                    if created.elapsed().as_secs() < 6 {
                        let msg_color = if is_dark { Color32::from_rgb(180, 180, 180) } else { Color32::from_rgb(70, 75, 85) };
                        ui.label(RichText::new(msg).small().color(msg_color));
                    }
                } else {
                    let ready_color = if is_dark { Color32::from_rgb(120, 120, 120) } else { Color32::from_rgb(105, 110, 120) };
                    ui.label(RichText::new("Ready").small().color(ready_color));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "Active Sessions: {} | Profiles: {}",
                            self.sessions.len(),
                            self.profile_store.profiles.len()
                        ))
                        .small(),
                    );
                });
            });
        });

        // Left Sidebar: Connection Profiles & Bookmarks
        egui::SidePanel::left("left_profiles")
            .resizable(true)
            .default_width(250.0)
            .width_range(200.0..=400.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Connections").size(14.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("➕ New").clicked() {
                            self.modal_state.editing_profile = Some(ConnectionProfile::default());
                        }
                    });
                });

                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("🔍 Search profiles...")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                ui.separator();

                // Profiles list
                let mut connect_profile = None;
                let mut edit_profile = None;
                let mut delete_profile_id = None;

                let filter = self.search_query.to_lowercase();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for profile in &self.profile_store.profiles {
                        if !filter.is_empty()
                            && !profile.name.to_lowercase().contains(&filter)
                            && !profile.host.to_lowercase().contains(&filter)
                            && !profile.group.to_lowercase().contains(&filter)
                        {
                            continue;
                        }

                        let badge_c = Color32::from_rgb(
                            profile.badge_color[0],
                            profile.badge_color[1],
                            profile.badge_color[2],
                        );

                        ui.horizontal(|ui| {
                            // Badge indicator
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 18.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, badge_c);

                            let label_resp = ui.selectable_label(
                                false,
                                RichText::new(&profile.name).strong(),
                            );

                            if label_resp.double_clicked() {
                                connect_profile = Some(profile.clone());
                            }

                            label_resp.context_menu(|ui| {
                                if ui.button("Connect").clicked() {
                                    connect_profile = Some(profile.clone());
                                    ui.close_menu();
                                }
                                if ui.button("Edit").clicked() {
                                    edit_profile = Some(profile.clone());
                                    ui.close_menu();
                                }
                                if ui.button("Delete").clicked() {
                                    delete_profile_id = Some(profile.id.clone());
                                    ui.close_menu();
                                }
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("▶").on_hover_text("Connect").clicked() {
                                    connect_profile = Some(profile.clone());
                                }
                            });
                        });

                        let sub_color = if ui.visuals().dark_mode {
                            Color32::from_rgb(140, 140, 140)
                        } else {
                            Color32::from_rgb(95, 100, 110)
                        };
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new(format!("{}@{}:{}", profile.username, profile.host, profile.port))
                                    .small()
                                    .color(sub_color),
                            );
                        });
                        ui.add_space(4.0);
                    }

                    if self.profile_store.profiles.is_empty() {
                        ui.add_space(24.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("No saved connections yet").weak());
                            ui.add_space(6.0);
                            if ui.button("➕ Add Connection").clicked() {
                                self.modal_state.editing_profile = Some(ConnectionProfile::default());
                            }
                        });
                    }
                });

                if let Some(p) = connect_profile {
                    self.connect_to_profile(p);
                }
                if let Some(p) = edit_profile {
                    self.modal_state.editing_profile = Some(p);
                }
                if let Some(id) = delete_profile_id {
                    self.profile_store.delete(&id);
                }
            });

        // Toggle Toolbox panel with F2 shortcut
        if ctx.input(|i| i.key_pressed(Key::F2)) {
            self.show_toolbox = !self.show_toolbox;
        }

        // Right SidePanel: Command Toolbox (when terminal has active session)
        if self.view_mode == ViewMode::Terminal && self.show_toolbox && !self.sessions.is_empty() {
            egui::SidePanel::right("toolbox_sidepanel")
                .resizable(true)
                .default_width(330.0)
                .min_width(270.0)
                .max_width(520.0)
                .show(ctx, |ui| {
                    self.render_toolbox_panel(ui);
                });
        }

        // Main Center Area
        egui::CentralPanel::default().show(ctx, |ui| match self.view_mode {
            ViewMode::Terminal => self.render_terminal_view(ui),
            ViewMode::Sftp => self.render_sftp_view(ui),
            ViewMode::Tunnels => self.render_tunnels_view(ui),
            ViewMode::Settings => self.render_settings_view(ui),
        });

        // Handle Active Session Prompts (Fingerprints and Passwords)
        self.render_active_prompts(ctx);

        // Edit Profile Modal
        if let Some(mut profile) = self.modal_state.editing_profile.take() {
            match render_edit_profile_modal(ctx, &mut profile) {
                ProfileEditAction::Save(saved) => {
                    self.profile_store.add_or_update(saved);
                    self.set_status("Profile saved");
                }
                ProfileEditAction::Cancel => {
                    // Closed
                }
                ProfileEditAction::None => {
                    // Keep modal open
                    self.modal_state.editing_profile = Some(profile);
                }
            }
        }

        // Add/Edit Custom Command Modal
        if self.custom_cmd_draft.is_some() {
            self.render_custom_cmd_modal(ctx);
        }

        // Request repaint to keep terminal fluid at 60fps when active
        if !self.sessions.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}

impl FerroSshApp {
    fn render_terminal_view(&mut self, ui: &mut egui::Ui) {
        if self.sessions.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("No Active Sessions");
                    ui.label("Select a profile on the left or use Quick Connect to start an SSH session.");
                });
            });
            return;
        }

        // Tabs bar for active sessions
        ui.horizontal(|ui| {
            let mut close_idx = None;

            for (idx, session) in self.sessions.iter().enumerate() {
                let is_active = idx == self.active_session_idx;
                let title = format!("📟 {}", session.profile.name);

                ui.horizontal(|ui| {
                    if ui.selectable_label(is_active, RichText::new(title).strong()).clicked() {
                        self.active_session_idx = idx;
                    }
                    if ui.small_button("✖").clicked() {
                        close_idx = Some(idx);
                    }
                });
            }

            if let Some(idx) = close_idx {
                self.sessions.remove(idx);
                if self.active_session_idx >= self.sessions.len() && !self.sessions.is_empty() {
                    self.active_session_idx = self.sessions.len() - 1;
                }
            }
        });

        ui.separator();

        // Snippets quick bar & Toolbox toggle
        ui.horizontal(|ui| {
            ui.label(RichText::new("Quick:").small().strong());
            for &(name, cmd) in &[
                ("htop", "htop\n"),
                ("df -h", "df -h\n"),
                ("free -h", "free -h\n"),
                ("docker ps", "docker ps\n"),
                ("ss -tulpn", "ss -tulpn\n"),
                ("clear", "clear\n"),
            ] {
                if ui.small_button(name).clicked() {
                    if let Some(session) = self.sessions.get_mut(self.active_session_idx) {
                        session.send_input(cmd.as_bytes());
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let toggle_label = if self.show_toolbox { "🧰 Hide Toolbox (F2)" } else { "🧰 Command Toolbox (F2)" };
                if ui.selectable_label(self.show_toolbox, RichText::new(toggle_label).strong()).clicked() {
                    self.show_toolbox = !self.show_toolbox;
                }
            });
        });
        ui.separator();

        // Terminal Grid Rendering & Input Handling
        if let Some(session) = self.sessions.get_mut(self.active_session_idx) {
            let font_id = FontId::new(self.settings.terminal_font_size, FontFamily::Monospace);

            // Handle terminal input
            let events = ui.input(|i| i.events.clone());
            for event in events {
                match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        handle_key_event(session, key, modifiers);
                    }
                    egui::Event::Text(text) => {
                        session.send_input(text.as_bytes());
                    }
                    _ => {}
                }
            }

            egui::ScrollArea::both().show(ui, |ui| {
                let term_response = render_terminal_ui(ui, &mut session.screen, font_id);
                if term_response.clicked() {
                    term_response.request_focus();
                }
            });
        }
    }

    fn render_active_prompts(&mut self, ctx: &egui::Context) {
        if self.sessions.is_empty() || self.active_session_idx >= self.sessions.len() {
            return;
        }

        let prompt_opt = self.sessions[self.active_session_idx].active_prompt.clone();
        if let Some(prompt) = prompt_opt {
            match prompt {
                PromptType::HostFingerprint {
                    host,
                    port,
                    key_type,
                    fingerprint,
                } => {
                    let fp_prompt = FingerprintPrompt {
                        host,
                        port,
                        key_type,
                        fingerprint,
                    };
                    let action = render_fingerprint_modal(ctx, &fp_prompt);
                    match action {
                        FingerprintAction::AcceptAndSave => {
                            self.sessions[self.active_session_idx].answer_fingerprint(true, false);
                        }
                        FingerprintAction::AcceptOnce => {
                            self.sessions[self.active_session_idx].answer_fingerprint(false, true);
                        }
                        FingerprintAction::Reject => {
                            self.sessions[self.active_session_idx].answer_fingerprint(false, false);
                        }
                        FingerprintAction::None => {}
                    }
                }
                PromptType::Password { user, host } => {
                    let mut pass = self.modal_state.password_input.clone();
                    let action = render_password_modal(ctx, &user, &host, &mut pass);
                    self.modal_state.password_input = pass;

                    match action {
                        PasswordAction::Submit(secret) => {
                            self.sessions[self.active_session_idx].send_password(&secret);
                        }
                        PasswordAction::Cancel => {
                            self.sessions[self.active_session_idx].active_prompt = None;
                        }
                        PasswordAction::None => {}
                    }
                }
            }
        }
    }

    fn render_sftp_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("📁 SFTP Remote File Browser");
        ui.add_space(6.0);

        if self.sessions.is_empty() {
            ui.label("Connect to an SSH session to browse remote files.");
            return;
        }

        let session = &self.sessions[self.active_session_idx];

        if self.sftp_browser.is_none() {
            let mut browser = SftpBrowser::new(
                session.profile.host.clone(),
                session.profile.port,
                session.profile.username.clone(),
            );
            let _ = browser.refresh_directory();
            self.sftp_browser = Some(browser);
        }

        if let Some(browser) = &mut self.sftp_browser {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Remote Directory:").strong());
                ui.text_edit_singleline(&mut browser.current_path);

                if ui.button("🔄 Refresh").clicked() {
                    let _ = browser.refresh_directory();
                }

                if ui.button("⬆ Upload File...").clicked() {
                    if let Some(file) = rfd::FileDialog::new().pick_file() {
                        if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                            let _ = browser.upload_file(&file, name);
                            let _ = browser.refresh_directory();
                        }
                    }
                }
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("sftp_grid")
                    .num_columns(4)
                    .spacing([20.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Name").strong());
                        ui.label(RichText::new("Size").strong());
                        ui.label(RichText::new("Permissions").strong());
                        ui.label(RichText::new("Actions").strong());
                        ui.end_row();

                        let mut navigate_to = None;
                        let mut download_target = None;

                        for entry in &browser.entries {
                            let icon = if entry.is_dir { "📁" } else { "📄" };
                            let label = format!("{} {}", icon, entry.name);

                            if entry.is_dir {
                                if ui.link(label).clicked() {
                                    navigate_to = Some(entry.name.clone());
                                }
                            } else {
                                ui.label(label);
                            }

                            let size_str = if entry.is_dir {
                                "-".to_string()
                            } else {
                                format!("{:.1} KB", entry.size as f64 / 1024.0)
                            };
                            ui.label(size_str);
                            ui.label(RichText::new(&entry.permissions).monospace());

                            if !entry.is_dir {
                                if ui.small_button("⬇ Download").clicked() {
                                    download_target = Some(entry.name.clone());
                                }
                            } else {
                                ui.label("");
                            }

                            ui.end_row();
                        }

                        if let Some(folder) = navigate_to {
                            browser.navigate(&folder);
                        }
                        if let Some(file_name) = download_target {
                            if let Some(dest) = rfd::FileDialog::new().set_file_name(&file_name).save_file() {
                                let _ = browser.download_file(&file_name, &dest);
                            }
                        }
                    });
            });
        }
    }

    fn render_tunnels_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("🔀 SSH Port Forwarding & Tunnels");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("➕ Add Local Forward (-L)").clicked() {
                self.tunnel_manager.tunnels.push(PortTunnel::new(
                    "Local Service Forward",
                    8080,
                    "127.0.0.1",
                    8080,
                    "user",
                    "remote.example.com",
                    22,
                ));
            }
        });

        ui.add_space(10.0);
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut start_id = None;
            let mut stop_id = None;

            if self.tunnel_manager.tunnels.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No active port forwards configured.").weak());
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Click '+ Add Local Forward (-L)' above to create a local SSH port forward.")
                            .small()
                            .weak(),
                    );
                });
            }

            for tunnel in &self.tunnel_manager.tunnels {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let status_dot = if tunnel.is_active { "🟢 Active" } else { "⚪ Stopped" };
                        ui.label(RichText::new(status_dot).strong());
                        ui.label(RichText::new(&tunnel.name).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if tunnel.is_active {
                                if ui.button("⏹ Stop").clicked() {
                                    stop_id = Some(tunnel.id.clone());
                                }
                            } else if ui.button("▶ Start").clicked() {
                                start_id = Some(tunnel.id.clone());
                            }
                        });
                    });

                    ui.label(format!(
                        "Local Port: {}  ➡  Forward to: {}:{} (via {}@{})",
                        tunnel.bind_port, tunnel.target_host, tunnel.target_port, tunnel.ssh_user, tunnel.ssh_host
                    ));
                });
            }

            if let Some(id) = start_id {
                let _ = self.tunnel_manager.start_tunnel(&id);
            }
            if let Some(id) = stop_id {
                self.tunnel_manager.stop_tunnel(&id);
            }
        });
    }

    fn render_settings_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙ Settings & Security");
        ui.add_space(12.0);

        ui.group(|ui| {
            ui.label(RichText::new("Theme & Visual Appearance").strong());
            ui.add_space(6.0);

            let prev_theme = self.settings.theme_mode;
            ui.horizontal(|ui| {
                ui.label("Theme:");
                ui.radio_value(&mut self.settings.theme_mode, ThemeMode::System, "System (Auto)");
                ui.radio_value(&mut self.settings.theme_mode, ThemeMode::Dark, "Dark Mode");
                ui.radio_value(&mut self.settings.theme_mode, ThemeMode::Light, "Light Mode");
            });

            let active_desc = match self.settings.theme_mode {
                ThemeMode::System => {
                    let active_name = if self.last_applied_dark.unwrap_or(true) { "Dark" } else { "Light" };
                    format!("Matches operating system preference dynamically (currently active: {}).", active_name)
                }
                ThemeMode::Dark => "Always uses dark theme.".to_string(),
                ThemeMode::Light => "Always uses light theme.".to_string(),
            };
            ui.label(RichText::new(active_desc).small().weak());

            if self.settings.theme_mode != prev_theme {
                self.settings.save();
                self.last_applied_dark = None; // trigger instant redraw
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Terminal Font Size:");
                let prev_font = self.settings.terminal_font_size;
                ui.add(egui::Slider::new(&mut self.settings.terminal_font_size, 10.0..=24.0).text("pt"));
                if (self.settings.terminal_font_size - prev_font).abs() > 0.01 {
                    self.settings.save();
                }
            });
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.label(RichText::new("SSH Key Management (~/.ssh/)").strong());
            ui.add_space(6.0);
            ui.label("Detected private keys on your system:");

            for key in &self.available_keys {
                ui.label(RichText::new(format!("  🔑 {}", key.display())).monospace());
            }

            if self.available_keys.is_empty() {
                ui.label("  No standard private keys found in ~/.ssh/.");
            }
        });

        ui.add_space(12.0);
        ui.group(|ui| {
            ui.label(RichText::new("Known Hosts & Trust On First Use").strong());
            ui.add_space(6.0);
            ui.label("Known hosts are strictly verified against ~/.ssh/known_hosts.");
            ui.label("When an unrecognized host fingerprint is encountered, a confirmation prompt allows you to Accept & Save or Reject.");
        });
    }

    fn render_toolbox_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🧰 Command Toolbox").strong().size(15.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✖").on_hover_text("Close Toolbox (F2)").clicked() {
                    self.show_toolbox = false;
                }
                if ui.small_button("➕ Custom").on_hover_text("Add custom command snippet").clicked() {
                    self.custom_cmd_draft = Some(CustomCmdDraft {
                        category: "Custom".to_string(),
                        ..Default::default()
                    });
                }
            });
        });

        ui.add_space(6.0);

        // Search bar
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut self.toolbox_search)
                    .hint_text("Search (e.g. disk, docker, ports)...")
                    .desired_width(ui.available_width() - 30.0),
            );
            if !self.toolbox_search.is_empty() {
                if ui.small_button("✖").clicked() {
                    self.toolbox_search.clear();
                }
            }
        });

        ui.add_space(4.0);

        // Category pills
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for cat in &ToolboxCategory::ALL {
                    let is_selected = self.toolbox_category == *cat;
                    let label = format!("{} {}", cat.icon(), cat.as_str());
                    if ui.selectable_label(is_selected, label).clicked() {
                        self.toolbox_category = *cat;
                    }
                }
            });
        });

        ui.separator();

        let filter = self.toolbox_search.trim().to_lowercase();
        let current_cat = self.toolbox_category;

        let mut send_cmd: Option<String> = None;
        let mut insert_cmd: Option<String> = None;
        let mut delete_id: Option<String> = None;
        let mut copy_status: Option<String> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut count = 0;
            for cmd in self.toolbox.all_commands() {
                if current_cat != ToolboxCategory::All {
                    if current_cat == ToolboxCategory::Custom && !cmd.is_custom {
                        continue;
                    }
                    if current_cat != ToolboxCategory::Custom && !current_cat.matches(&cmd.category) {
                        continue;
                    }
                }

                if !filter.is_empty() {
                    let matches_title = cmd.title.to_lowercase().contains(&filter);
                    let matches_cmd = cmd.command.to_lowercase().contains(&filter);
                    let matches_desc = cmd.description.to_lowercase().contains(&filter);
                    let matches_cat = cmd.category.to_lowercase().contains(&filter);
                    if !matches_title && !matches_cmd && !matches_desc && !matches_cat {
                        continue;
                    }
                }

                count += 1;

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&cmd.title).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let cat_color = match cmd.category.as_str() {
                                "System" => Color32::from_rgb(100, 180, 255),
                                "Disk" => Color32::from_rgb(255, 180, 80),
                                "Network" => Color32::from_rgb(120, 220, 120),
                                "Docker" => Color32::from_rgb(70, 200, 240),
                                "Services" => Color32::from_rgb(230, 140, 240),
                                "Security" => Color32::from_rgb(255, 110, 110),
                                _ => Color32::from_rgb(240, 220, 100),
                            };
                            ui.label(RichText::new(&cmd.category).small().color(cat_color));
                        });
                    });

                    let is_dark = ui.visuals().dark_mode;
                    let mut cmd_preview = cmd.command.clone();
                    let code_color = if is_dark {
                        Color32::from_rgb(175, 230, 175)
                    } else {
                        Color32::from_rgb(20, 115, 60)
                    };
                    ui.add(
                        egui::TextEdit::singleline(&mut cmd_preview)
                            .font(FontId::new(11.0, FontFamily::Monospace))
                            .text_color(code_color)
                            .desired_width(ui.available_width())
                            .interactive(false),
                    );

                    if !cmd.description.is_empty() {
                        let desc_color = if is_dark {
                            Color32::from_rgb(150, 150, 150)
                        } else {
                            Color32::from_rgb(95, 100, 110)
                        };
                        ui.label(
                            RichText::new(&cmd.description)
                                .small()
                                .color(desc_color),
                        );
                    }

                    ui.horizontal(|ui| {
                        if ui
                            .button("▶ Run")
                            .on_hover_text("Execute command immediately with newline")
                            .clicked()
                        {
                            send_cmd = Some(format!("{}\n", cmd.command));
                        }

                        if ui
                            .button("✏ Insert")
                            .on_hover_text("Insert at terminal cursor without Enter so you can edit args")
                            .clicked()
                        {
                            insert_cmd = Some(cmd.command.clone());
                        }

                        if ui
                            .small_button("📋 Copy")
                            .on_hover_text("Copy command text to clipboard")
                            .clicked()
                        {
                            ui.output_mut(|o| o.copied_text = cmd.command.clone());
                            copy_status = Some(format!("Copied: {}", cmd.title));
                        }

                        if cmd.is_custom {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .small_button("🗑")
                                    .on_hover_text("Delete custom command")
                                    .clicked()
                                {
                                    delete_id = Some(cmd.id.clone());
                                }
                            });
                        }
                    });
                });

                ui.add_space(3.0);
            }

            if count == 0 {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No matching commands found.").color(Color32::GRAY));
                    if !self.toolbox_search.is_empty() {
                        if ui.button("Clear search filter").clicked() {
                            self.toolbox_search.clear();
                        }
                    }
                });
            }
        });

        if let Some(cmd) = send_cmd {
            if let Some(session) = self.sessions.get_mut(self.active_session_idx) {
                session.send_input(cmd.as_bytes());
            }
        } else if let Some(cmd) = insert_cmd {
            if let Some(session) = self.sessions.get_mut(self.active_session_idx) {
                session.send_input(cmd.as_bytes());
            }
        }

        if let Some(id) = delete_id {
            self.toolbox.delete_custom(&id);
            self.set_status("Deleted custom command");
        }

        if let Some(msg) = copy_status {
            self.set_status(&msg);
        }
    }

    fn render_custom_cmd_modal(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let mut save = false;

        if let Some(draft) = &mut self.custom_cmd_draft {
            egui::Window::new("➕ Add Custom Toolbox Command")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([440.0, 320.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new("Add a frequently used command or snippet to your personal toolbox:").small());
                    ui.add_space(8.0);

                    ui.label(RichText::new("Title:").strong());
                    ui.text_edit_singleline(&mut draft.title);

                    ui.add_space(4.0);
                    ui.label(RichText::new("Category:").strong());
                    egui::ComboBox::from_id_source("custom_cmd_category_box")
                        .selected_text(&draft.category)
                        .show_ui(ui, |ui| {
                            for cat in &[
                                "System", "Disk", "Network", "Docker", "Services", "Security", "Custom",
                            ] {
                                ui.selectable_value(&mut draft.category, cat.to_string(), *cat);
                            }
                        });

                    ui.add_space(4.0);
                    ui.label(RichText::new("Command:").strong());
                    ui.add(
                        egui::TextEdit::multiline(&mut draft.command)
                            .font(FontId::new(12.0, FontFamily::Monospace))
                            .desired_rows(3)
                            .desired_width(ui.available_width()),
                    );

                    ui.add_space(4.0);
                    ui.label(RichText::new("Description (Optional):").strong());
                    ui.text_edit_singleline(&mut draft.description);

                    ui.add_space(12.0);
                    ui.separator();

                    ui.horizontal(|ui| {
                        let can_save = !draft.title.trim().is_empty() && !draft.command.trim().is_empty();
                        if ui.add_enabled(can_save, egui::Button::new("💾 Save Command")).clicked() {
                            save = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
        }

        if save {
            if let Some(draft) = self.custom_cmd_draft.take() {
                self.toolbox.add_custom(
                    draft.title.trim().to_string(),
                    draft.command.trim().to_string(),
                    draft.description.trim().to_string(),
                    if draft.category.trim().is_empty() {
                        "Custom".to_string()
                    } else {
                        draft.category
                    },
                );
                self.set_status("Custom command saved to Toolbox");
            }
        } else if close {
            self.custom_cmd_draft = None;
        }
    }
}

fn handle_key_event(session: &mut SshSession, key: Key, modifiers: Modifiers) {
    if modifiers.ctrl {
        match key {
            Key::C => session.send_input(&[0x03]), // Ctrl+C
            Key::D => session.send_input(&[0x04]), // Ctrl+D
            Key::Z => session.send_input(&[0x1a]), // Ctrl+Z
            Key::L => session.send_input(&[0x0c]), // Ctrl+L
            Key::A => session.send_input(&[0x01]), // Ctrl+A
            Key::E => session.send_input(&[0x05]), // Ctrl+E
            Key::U => session.send_input(&[0x15]), // Ctrl+U
            Key::K => session.send_input(&[0x0b]), // Ctrl+K
            Key::W => session.send_input(&[0x17]), // Ctrl+W
            _ => {}
        }
        return;
    }

    match key {
        Key::ArrowUp => session.send_input(b"\x1b[A"),
        Key::ArrowDown => session.send_input(b"\x1b[B"),
        Key::ArrowRight => session.send_input(b"\x1b[C"),
        Key::ArrowLeft => session.send_input(b"\x1b[D"),
        Key::Home => session.send_input(b"\x1b[H"),
        Key::End => session.send_input(b"\x1b[F"),
        Key::PageUp => session.send_input(b"\x1b[5~"),
        Key::PageDown => session.send_input(b"\x1b[6~"),
        Key::Backspace => session.send_input(&[0x7f]),
        Key::Enter => session.send_input(b"\r"),
        Key::Tab => session.send_input(b"\t"),
        Key::Escape => session.send_input(b"\x1b"),
        _ => {}
    }
}
