use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::obfuscation::admin_password_config_value;
use crate::overlay_columns::{default_order, normalize_order};

const CONFIG_DIR_NAME: &str = "shcde-monitor";
const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_description")]
    pub description: String,
    #[serde(default = "default_players")]
    pub players: Vec<PlayerSlotConfig>,
    #[serde(default = "default_true")]
    pub pin_on_top: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_password: Option<String>,
    pub settings_tab: i32,
    pub overlay: OverlaySettings,
    pub timer: TimerSettings,
    pub columns: ColumnVisibility,
    pub shooter_types: ShooterTypeVisibility,
    pub main_window: SavedWindowPosition,
    pub overlay_window: SavedWindowPosition,
    pub timer_window: SavedWindowPosition,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: default_title(),
            description: default_description(),
            players: default_players(),
            pin_on_top: default_true(),
            admin_password: None,
            settings_tab: 0,
            overlay: OverlaySettings::default(),
            timer: TimerSettings::default(),
            columns: ColumnVisibility::default(),
            shooter_types: ShooterTypeVisibility::default(),
            main_window: SavedWindowPosition::default(),
            overlay_window: SavedWindowPosition::default(),
            timer_window: SavedWindowPosition::default(),
        }
    }
}

impl AppConfig {
    pub fn load_or_create() -> Self {
        let path = config_path();
        let mut config = fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str::<AppConfig>(&json).ok())
            .unwrap_or_default();

        config.normalize();
        let _ = config.save();
        config
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("could not create config directory {}", parent.display())
            })?;
        }

        let json = serde_json::to_string_pretty(self).context("could not encode config")?;
        fs::write(&path, json)
            .with_context(|| format!("could not write config file {}", path.display()))
    }

    pub fn normalize(&mut self) {
        let defaults = default_players();

        if self.title.trim().is_empty() {
            self.title = default_title();
        }
        if self.description.trim().is_empty() {
            self.description = default_description();
        }
        self.admin_password = self
            .admin_password
            .as_deref()
            .and_then(admin_password_config_value);

        if self.players.len() < 8 {
            self.players
                .extend(defaults.iter().skip(self.players.len()).cloned());
        }
        self.players.truncate(8);

        for (index, player) in self.players.iter_mut().enumerate() {
            if player.name.trim().is_empty() {
                player.name = defaults[index].name.clone();
            }
            if !(1..=8).contains(&player.color) {
                player.color = defaults[index].color;
            }
        }

        self.overlay.font_size = self.overlay.font_size.clamp(10.0, 26.0);
        self.overlay.panel_opacity = self.overlay.panel_opacity.clamp(0.25, 1.0);
        self.overlay.heavy_read_ms = self.overlay.heavy_read_ms.clamp(50.0, 1000.0);
        self.overlay.column_order = normalize_order(&self.overlay.column_order);
        self.timer.font_size = self.timer.font_size.clamp(10.0, 28.0);
        self.timer.panel_opacity = self.timer.panel_opacity.clamp(0.25, 1.0);
        self.timer.game_speed = quantized_game_speed(self.timer.game_speed);
        self.settings_tab = self.settings_tab.clamp(0, 3);
        if !self.admin_mode_enabled() && self.settings_tab >= 2 {
            self.settings_tab = 0;
        }

        self.main_window.normalize();
        self.overlay_window.normalize();
        self.timer_window.normalize();
    }

    pub fn admin_mode_enabled(&self) -> bool {
        self.admin_password
            .as_deref()
            .is_some_and(|value| admin_password_config_value(value).is_some())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct PlayerSlotConfig {
    pub name: String,
    pub visible: bool,
    pub color: i32,
}

impl Default for PlayerSlotConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            visible: false,
            color: 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct OverlaySettings {
    #[serde(default = "default_panel_opacity")]
    pub panel_opacity: f32,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_heavy_read_ms")]
    pub heavy_read_ms: f32,
    #[serde(default = "default_true")]
    pub movable_frame: bool,
    #[serde(
        default = "default_order",
        deserialize_with = "deserialize_column_order"
    )]
    pub column_order: Vec<String>,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            panel_opacity: default_panel_opacity(),
            font_size: default_font_size(),
            heavy_read_ms: default_heavy_read_ms(),
            movable_frame: default_true(),
            column_order: default_order(),
        }
    }
}

fn deserialize_column_order<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_else(default_order))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TimerSettings {
    #[serde(default = "default_panel_opacity")]
    pub panel_opacity: f32,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_game_speed")]
    pub game_speed: f32,
    #[serde(default = "default_true")]
    pub movable_frame: bool,
    pub show_buildings: bool,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            panel_opacity: default_panel_opacity(),
            font_size: default_font_size(),
            game_speed: default_game_speed(),
            movable_frame: default_true(),
            show_buildings: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ColumnVisibility {
    #[serde(default = "default_true")]
    pub gold: bool,
    #[serde(default = "default_true")]
    pub population: bool,
    #[serde(default = "default_true")]
    pub army: bool,
    #[serde(default = "default_true")]
    pub food_effect: bool,
    #[serde(default = "default_true")]
    pub tax_effect: bool,
    #[serde(default = "default_true")]
    pub ha: bool,
    #[serde(default = "default_true")]
    pub cl: bool,
    pub shooters: bool,
    pub acquired_gold: bool,
    pub produced_weapons: bool,
    pub produced_food: bool,
    pub produced_stone: bool,
    pub produced_iron: bool,
    pub produced_wood: bool,
    pub total_effect: bool,
    #[serde(default = "default_true")]
    pub knights: bool,
    pub fear_factor: bool,
    pub army_killed: bool,
    pub army_lost: bool,
}

impl Default for ColumnVisibility {
    fn default() -> Self {
        Self {
            gold: true,
            population: true,
            army: true,
            food_effect: true,
            tax_effect: true,
            ha: true,
            cl: true,
            shooters: false,
            acquired_gold: false,
            produced_weapons: false,
            produced_food: false,
            produced_stone: false,
            produced_iron: false,
            produced_wood: false,
            total_effect: false,
            knights: true,
            fear_factor: false,
            army_killed: false,
            army_lost: false,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ShooterTypeVisibility {
    pub skirmishers: bool,
    pub archers: bool,
    pub xbows: bool,
    pub arab_bows: bool,
    pub slingers: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SavedWindowPosition {
    pub saved: bool,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl SavedWindowPosition {
    pub fn is_usable(&self) -> bool {
        self.saved && is_usable_screen_coordinate(self.x) && is_usable_screen_coordinate(self.y)
    }

    pub fn normalize(&mut self) {
        if !self.is_usable() {
            *self = Self::default();
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Some(appdata) = env::var_os("APPDATA") {
        return PathBuf::from(appdata)
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME);
    }

    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home)
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME);
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(CONFIG_FILE_NAME)
}

fn default_title() -> String {
    "Title".to_string()
}

fn default_description() -> String {
    "Description (score, etc.)".to_string()
}

fn default_true() -> bool {
    true
}

fn default_panel_opacity() -> f32 {
    0.78
}

fn default_font_size() -> f32 {
    16.0
}

fn default_heavy_read_ms() -> f32 {
    200.0
}

fn default_game_speed() -> f32 {
    50.0
}

fn default_players() -> Vec<PlayerSlotConfig> {
    (0..8)
        .map(|index| PlayerSlotConfig {
            name: format!("Player {}", index + 1),
            visible: index < 4,
            color: (index + 1) as i32,
        })
        .collect()
}

fn quantized_game_speed(value: f32) -> f32 {
    ((value / 5.0).round() * 5.0).clamp(40.0, 90.0)
}

fn is_usable_screen_coordinate(value: i32) -> bool {
    (-10_000..=30_000).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_admin_tabs_require_a_valid_admin_key() {
        for tab in [2, 3, 100] {
            for password in [None, Some("invalid-key".to_owned())] {
                let mut config = AppConfig {
                    settings_tab: tab,
                    admin_password: password,
                    ..AppConfig::default()
                };
                config.normalize();
                assert_eq!(config.settings_tab, 0);
                assert!(!config.admin_mode_enabled());
            }
        }
        let mut config = AppConfig {
            settings_tab: 1,
            ..AppConfig::default()
        };
        config.normalize();
        assert_eq!(config.settings_tab, 1);
    }

    #[test]
    fn existing_settings_gain_default_order_without_changing_visibility() {
        let mut config: AppConfig = serde_json::from_value(serde_json::json!({
            "title": "Custom match title",
            "overlay": { "font_size": 21.0, "panel_opacity": 0.6 },
            "columns": { "gold": false, "army": true, "produced_iron": true }
        }))
        .unwrap();
        config.normalize();

        assert_eq!(config.overlay.column_order, default_order());
        assert_eq!(config.overlay.font_size, 21.0);
        assert_eq!(config.overlay.panel_opacity, 0.6);
        assert_eq!(config.title, "Custom match title");
        assert!(!config.columns.gold);
        assert!(config.columns.army);
        assert!(config.columns.produced_iron);
    }

    #[test]
    fn malformed_column_order_does_not_reset_other_settings() {
        for malformed in [
            serde_json::Value::Null,
            serde_json::json!(42),
            serde_json::json!("gold"),
            serde_json::json!({ "gold": 1 }),
        ] {
            let mut config: AppConfig = serde_json::from_value(serde_json::json!({
                "title": "Keep this title",
                "overlay": { "font_size": 20.0, "column_order": malformed },
                "columns": { "gold": false, "fear_factor": true },
                "timer": { "game_speed": 65.0 }
            }))
            .unwrap();
            config.normalize();

            assert_eq!(config.title, "Keep this title");
            assert_eq!(config.overlay.font_size, 20.0);
            assert_eq!(config.timer.game_speed, 65.0);
            assert!(!config.columns.gold);
            assert!(config.columns.fear_factor);
            assert_eq!(config.overlay.column_order, default_order());
        }
    }

    #[test]
    fn mixed_column_order_retains_valid_unique_entries_and_adds_missing_columns() {
        let mut config: AppConfig = serde_json::from_value(serde_json::json!({
            "overlay": {
                "column_order": ["army", 7, "gold", null, "future_column", "army"]
            }
        }))
        .unwrap();
        config.normalize();

        assert_eq!(&config.overlay.column_order[..2], &["army", "gold"]);
        assert_eq!(config.overlay.column_order.len(), default_order().len());
        assert_eq!(
            config.overlay.column_order,
            normalize_order(&["army".to_owned(), "gold".to_owned()])
        );
    }

    #[test]
    fn custom_column_order_and_hidden_columns_survive_settings_round_trip() {
        let mut config = AppConfig::default();
        config.overlay.column_order =
            normalize_order(&["army_lost".to_owned(), "produced_iron".to_owned()]);
        config.columns.gold = false;
        config.columns.produced_iron = true;
        config.columns.army_lost = false;
        config.players[0].name = "Custom player".to_owned();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let mut restored: AppConfig = serde_json::from_str(&json).unwrap();
        restored.normalize();

        assert_eq!(restored.overlay.column_order, config.overlay.column_order);
        assert_eq!(restored.players[0].name, "Custom player");
        assert!(!restored.columns.gold);
        assert!(restored.columns.produced_iron);
        assert!(!restored.columns.army_lost);
        assert!(restored
            .overlay
            .column_order
            .iter()
            .any(|key| key == "gold"));
    }
}
