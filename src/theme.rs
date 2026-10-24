use eframe::egui::{self, Color32, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeMode {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            Self::System => "System (Auto)",
            Self::Dark => "Dark Mode",
            Self::Light => "Light Mode",
        }
    }

    /// Evaluates whether dark visuals should be applied based on user preference
    /// and the detected operating system theme.
    pub fn is_dark(&self, system_theme: Option<eframe::Theme>) -> bool {
        match self {
            Self::Dark => true,
            Self::Light => false,
            Self::System => match system_theme {
                Some(eframe::Theme::Light) => false,
                Some(eframe::Theme::Dark) => true,
                None => true, // Default to dark theme if OS theme cannot be determined
            },
        }
    }
}

/// Builds polished dark mode visuals matching modern terminal styling
pub fn build_dark_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(22, 24, 29);
    visuals.window_fill = Color32::from_rgb(28, 32, 38);
    visuals.extreme_bg_color = Color32::from_rgb(16, 18, 22);

    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 32, 38);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(205, 210, 220));
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(45, 50, 60));

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 40, 48);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(190, 195, 205));
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(50, 56, 68));

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 54, 66);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(240, 245, 255));
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(70, 80, 100));

    visuals.widgets.active.bg_fill = Color32::from_rgb(60, 68, 82);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(255, 255, 255));
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(52, 152, 219));

    visuals.selection.bg_fill = Color32::from_rgb(52, 152, 219);
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);

    visuals
}

/// Builds polished light mode visuals matching modern system desktop styling
pub fn build_light_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = Color32::from_rgb(246, 248, 252);
    visuals.window_fill = Color32::from_rgb(255, 255, 255);
    visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);

    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(240, 243, 248);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(35, 40, 50));
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(218, 222, 230));

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(232, 236, 244);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 65));
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(205, 212, 222));

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(220, 226, 238);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(20, 25, 35));
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(175, 185, 200));

    visuals.widgets.active.bg_fill = Color32::from_rgb(205, 214, 230);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(10, 15, 25));
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(41, 128, 185));

    visuals.selection.bg_fill = Color32::from_rgb(52, 152, 219);
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);

    visuals
}

/// Persistent user settings covering theme preference and terminal font size
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme_mode: ThemeMode,
    pub terminal_font_size: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            terminal_font_size: 13.0,
        }
    }
}

impl AppSettings {
    fn file_path() -> Option<std::path::PathBuf> {
        directories::BaseDirs::new()
            .map(|b| b.config_dir().join("ferro-ssh").join("settings.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::file_path() {
            if path.exists() {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(settings) = serde_json::from_str(&data) {
                        return settings;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::file_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(&path, json);
            }
        }
    }
}
