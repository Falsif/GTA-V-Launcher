//! Настройки лаунчера и сохранение их в JSON (%APPDATA%\com.lunexa.gtavlauncher\config.json).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::util::{CmdResult, Ctx};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    #[default]
    Legacy,
    Enhanced,
}

impl Edition {
    pub const ALL: [Edition; 2] = [Edition::Legacy, Edition::Enhanced];

    pub fn steam_appid(self) -> u32 {
        match self {
            Edition::Legacy => 271590,
            Edition::Enhanced => 3240220,
        }
    }

    /// Возможные имена исполняемого файла игры.
    pub fn exe_names(self) -> &'static [&'static str] {
        match self {
            Edition::Legacy => &["GTA5.exe"],
            Edition::Enhanced => &["GTA5_Enhanced.exe", "GTA5 Enhanced.exe"],
        }
    }

    /// Папка в «Документы\Rockstar Games».
    pub fn documents_folder(self) -> &'static str {
        match self {
            Edition::Legacy => "GTA V",
            Edition::Enhanced => "GTAV Enhanced",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Edition::Legacy => "GTA V Legacy",
            Edition::Enhanced => "GTA V Enhanced",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Steam,
    Epic,
    Rockstar,
    #[default]
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct GameInstall {
    pub path: String,
    pub source: Source,
    /// Для Epic: `Namespace:CatalogItemId:AppName`.
    pub epic_id: Option<String>,
    /// Путь найден автоматически (а не выбран вручную).
    pub auto: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Installs {
    pub legacy: Option<GameInstall>,
    pub enhanced: Option<GameInstall>,
}

impl Installs {
    pub fn get(&self, e: Edition) -> Option<&GameInstall> {
        match e {
            Edition::Legacy => self.legacy.as_ref(),
            Edition::Enhanced => self.enhanced.as_ref(),
        }
    }
    pub fn set(&mut self, e: Edition, v: Option<GameInstall>) {
        match e {
            Edition::Legacy => self.legacy = v,
            Edition::Enhanced => self.enhanced = v,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct EditionStats {
    /// Время игры, которое лаунчер видел (в секундах).
    pub seconds: u64,
    pub launches: u64,
    pub last_launch: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Stats {
    pub legacy: EditionStats,
    pub enhanced: EditionStats,
}

impl Stats {
    pub fn get_mut(&mut self, e: Edition) -> &mut EditionStats {
        match e {
            Edition::Legacy => &mut self.legacy,
            Edition::Enhanced => &mut self.enhanced,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct LauncherSettings {
    pub theme: String,
    pub has_background: bool,
    pub background_dim: u8,
    pub close_to_tray: bool,
    pub minimize_on_launch: bool,
    pub warn_mods_before_launch: bool,
    pub check_updates_on_start: bool,
    pub load_news: bool,
    pub screenshot_hotkey: bool,
    pub reduce_animations: bool,
    pub update_repo: String,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            theme: "red".into(),
            has_background: false,
            background_dim: 70,
            close_to_tray: false,
            minimize_on_launch: true,
            warn_mods_before_launch: true,
            check_updates_on_start: true,
            load_news: true,
            screenshot_hotkey: true,
            reduce_animations: false,
            update_repo: "Falsif/GTA-V-Launcher".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Эмодзи или data:image/...;base64
    pub avatar: String,
    pub edition: Edition,
    pub theme: String,
    /// Набор модов профиля (id). `None` — профиль не управляет модами.
    pub mod_ids: Option<Vec<String>>,
    pub launches: u64,
    pub seconds: u64,
    pub last_launch: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub category: String,
    pub edition: Edition,
    pub enabled: bool,
    /// Относительные пути (от корня игры) через `/`.
    pub files: Vec<String>,
    pub size: u64,
    pub installed_at: String,
    pub source_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Config {
    pub selected_edition: Edition,
    pub installs: Installs,
    pub stats: Stats,
    pub launcher: LauncherSettings,
    pub profiles: Vec<Profile>,
    pub active_profile: Option<String>,
    pub mods: Vec<ModEntry>,
    pub first_run_done: bool,
}

impl Config {
    pub fn active_profile_mut(&mut self) -> Option<&mut Profile> {
        let id = self.active_profile.clone()?;
        self.profiles.iter_mut().find(|p| p.id == id)
    }
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub config: Mutex<Config>,
}

impl AppState {
    pub fn load(data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&data_dir);
        for sub in ["mods", "backups", "config_backups", "settings_profiles"] {
            let _ = fs::create_dir_all(data_dir.join(sub));
        }
        let path = data_dir.join("config.json");
        let config = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
            .unwrap_or_else(|| {
                // Повреждённый файл сохраняем рядом, чтобы не потерять данные.
                if path.exists() {
                    let _ = fs::copy(&path, data_dir.join("config.broken.json"));
                }
                Config::default()
            });
        Self {
            data_dir,
            config: Mutex::new(config),
        }
    }

    pub fn snapshot(&self) -> Config {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Изменить конфигурацию и сразу сохранить на диск.
    pub fn update<R>(&self, f: impl FnOnce(&mut Config) -> R) -> CmdResult<R> {
        let mut guard = self.config.lock().unwrap_or_else(|e| e.into_inner());
        let r = f(&mut guard);
        save_config(&self.data_dir, &guard)?;
        Ok(r)
    }

    pub fn path(&self, sub: &str) -> PathBuf {
        self.data_dir.join(sub)
    }
}

pub fn save_config(dir: &Path, cfg: &Config) -> CmdResult<()> {
    let json = serde_json::to_string_pretty(cfg).ctx("Ошибка сериализации настроек")?;
    let tmp = dir.join("config.json.tmp");
    fs::write(&tmp, json).ctx("Не удалось записать настройки")?;
    fs::rename(&tmp, dir.join("config.json")).ctx("Не удалось сохранить настройки")?;
    Ok(())
}
