use crate::profiles::{AuthMethod, ConnectionProfile};
use crate::security::SecureSecret;
use eframe::egui::{self, Color32, RichText, Vec2};

pub struct ModalState {
    pub editing_profile: Option<ConnectionProfile>,
    pub password_input: String,
}

#[derive(Clone, Debug)]
pub struct FingerprintPrompt {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FingerprintAction {
    None,
    AcceptAndSave,
    AcceptOnce,
    Reject,
}

#[derive(Clone, Debug)]
pub enum PasswordAction {
    None,
    Submit(SecureSecret),
    Cancel,
}

#[derive(Clone, Debug)]
pub enum ProfileEditAction {
    None,
    Save(ConnectionProfile),
    Cancel,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            editing_profile: None,
            password_input: String::new(),
        }
    }
}

pub fn render_fingerprint_modal(
    ctx: &egui::Context,
    prompt: &FingerprintPrompt,
) -> FingerprintAction {
    let mut action = FingerprintAction::None;

    egui::Window::new("🛡️ Host Key Verification")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(480.0, 300.0))
        .show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading(RichText::new("Unrecognized Host Fingerprint").color(Color32::from_rgb(241, 196, 15)));
            ui.add_space(6.0);

            ui.label(
                "The authenticity of this remote host cannot be established from your known_hosts. \
                 This typically happens the first time you connect to a new server.",
            );
            ui.add_space(10.0);

            egui::Frame::group(ui.style())
                .fill(Color32::from_rgb(26, 29, 36))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Host:").strong());
                        ui.label(format!("{}:{}", prompt.host, prompt.port));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Key Algorithm:").strong());
                        ui.label(&prompt.key_type);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("SHA-256 Fingerprint:").strong());
                    });
                    ui.label(
                        RichText::new(&prompt.fingerprint)
                            .monospace()
                            .color(Color32::from_rgb(46, 204, 113)),
                    );
                });

            ui.add_space(12.0);
            ui.label(
                RichText::new("Are you sure you want to continue connecting?")
                    .small()
                    .italics(),
            );
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("✔ Accept & Save to Known Hosts").strong().color(Color32::WHITE))
                    .clicked()
                {
                    action = FingerprintAction::AcceptAndSave;
                }

                if ui.button("Accept Once (Session Only)").clicked() {
                    action = FingerprintAction::AcceptOnce;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new("✖ Reject & Abort").color(Color32::from_rgb(231, 76, 60)))
                        .clicked()
                    {
                        action = FingerprintAction::Reject;
                    }
                });
            });
        });

    action
}

pub fn render_password_modal(
    ctx: &egui::Context,
    user: &str,
    host: &str,
    password_input: &mut String,
) -> PasswordAction {
    let mut action = PasswordAction::None;

    egui::Window::new("🔐 Authentication Required")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(400.0, 180.0))
        .show(ctx, |ui| {
            ui.add_space(6.0);
            ui.label(format!("Please enter the password for {}@{}:", user, host));
            ui.add_space(8.0);

            let edit_response = ui.add(
                egui::TextEdit::singleline(password_input)
                    .password(true)
                    .hint_text("Password...")
                    .desired_width(f32::INFINITY),
            );

            // Auto-focus the password field
            if !edit_response.has_focus() {
                edit_response.request_focus();
            }

            let enter_pressed = edit_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Connect / Send Password").strong().color(Color32::WHITE))
                    .clicked()
                    || enter_pressed
                {
                    let secret = SecureSecret::new(std::mem::take(password_input));
                    action = PasswordAction::Submit(secret);
                }

                if ui.button("Cancel").clicked() {
                    password_input.clear();
                    action = PasswordAction::Cancel;
                }
            });
        });

    action
}

pub fn render_edit_profile_modal(
    ctx: &egui::Context,
    profile: &mut ConnectionProfile,
) -> ProfileEditAction {
    let mut action = ProfileEditAction::None;

    egui::Window::new("✏️ Connection Profile")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(520.0, 480.0))
        .show(ctx, |ui| {
            egui::Grid::new("profile_edit_grid")
                .num_columns(2)
                .spacing([12.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Profile Name:");
                    ui.text_edit_singleline(&mut profile.name);
                    ui.end_row();

                    ui.label("Host / IP:");
                    ui.text_edit_singleline(&mut profile.host);
                    ui.end_row();

                    ui.label("Port:");
                    let mut port_str = profile.port.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut port_str).desired_width(80.0)).changed() {
                        if let Ok(p) = port_str.parse::<u16>() {
                            profile.port = p;
                        }
                    }
                    ui.end_row();

                    ui.label("Username:");
                    ui.text_edit_singleline(&mut profile.username);
                    ui.end_row();

                    ui.label("Authentication:");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut profile.auth_method, AuthMethod::Password, "Password");
                        ui.radio_value(&mut profile.auth_method, AuthMethod::KeyFile, "SSH Key");
                        ui.radio_value(&mut profile.auth_method, AuthMethod::Agent, "SSH Agent");
                    });
                    ui.end_row();

                    if profile.auth_method == AuthMethod::KeyFile {
                        ui.label("Private Key File:");
                        ui.horizontal(|ui| {
                            let mut path_str = profile
                                .key_path
                                .as_ref()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();

                            if ui.text_edit_singleline(&mut path_str).changed() {
                                profile.key_path = if path_str.trim().is_empty() {
                                    None
                                } else {
                                    Some(std::path::PathBuf::from(path_str))
                                };
                            }

                            if ui.button("Browse...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_title("Select SSH Private Key")
                                    .pick_file()
                                {
                                    profile.key_path = Some(path);
                                }
                            }
                        });
                        ui.end_row();
                    }

                    ui.label("Group / Folder:");
                    ui.text_edit_singleline(&mut profile.group);
                    ui.end_row();

                    ui.label("Badge Color:");
                    ui.horizontal(|ui| {
                        let mut rgb = [
                            profile.badge_color[0] as f32 / 255.0,
                            profile.badge_color[1] as f32 / 255.0,
                            profile.badge_color[2] as f32 / 255.0,
                        ];
                        if egui::color_picker::color_edit_button_rgb(ui, &mut rgb).changed() {
                            profile.badge_color = [
                                (rgb[0] * 255.0) as u8,
                                (rgb[1] * 255.0) as u8,
                                (rgb[2] * 255.0) as u8,
                            ];
                        }
                    });
                    ui.end_row();

                    ui.label("Notes:");
                    ui.add(egui::TextEdit::multiline(&mut profile.notes).desired_rows(3).desired_width(f32::INFINITY));
                    ui.end_row();
                });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button(RichText::new("Save Profile").strong().color(Color32::WHITE)).clicked() {
                    action = ProfileEditAction::Save(profile.clone());
                }
                if ui.button("Cancel").clicked() {
                    action = ProfileEditAction::Cancel;
                }
            });
        });

    action
}
