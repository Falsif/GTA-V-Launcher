//! Поиск установленной GTA V: Steam, Epic Games, Rockstar Games Launcher и ручной выбор папки.

use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use crate::config::{Edition, GameInstall, Source};
use crate::util::{file_exists_ci, reg_query_value, reg_subkeys, same_path};

#[derive(Serialize, Clone, Debug)]
pub struct Found {
    pub edition: Edition,
    pub install: GameInstall,
}

/// Какая редакция лежит в папке. Enhanced проверяем первой: некоторые
/// инструменты кладут в её папку пустой GTA5.exe.
pub fn edition_of(dir: &Path) -> Option<Edition> {
    if Edition::Enhanced
        .exe_names()
        .iter()
        .any(|n| file_exists_ci(dir, n))
    {
        return Some(Edition::Enhanced);
    }
    if file_exists_ci(dir, "GTA5.exe") {
        // Пустой «обманный» GTA5.exe (OpenIV fix) — не настоящая Legacy.
        let real = fs::metadata(dir.join("GTA5.exe"))
            .map(|m| m.len() > 1_000_000)
            .unwrap_or(false);
        if real {
            return Some(Edition::Legacy);
        }
    }
    None
}

pub fn exe_path(dir: &Path, edition: Edition) -> Option<PathBuf> {
    edition
        .exe_names()
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

/// Файлы, без которых игра точно не запустится.
pub fn missing_files(dir: &Path, edition: Edition) -> Vec<String> {
    let mut missing = vec![];
    if !dir.is_dir() {
        missing.push(format!("Папка не найдена: {}", dir.display()));
        return missing;
    }
    if exe_path(dir, edition).is_none() {
        missing.push(edition.exe_names().join(" / "));
    }
    let mut required = vec!["PlayGTAV.exe", "common.rpf", "update/update.rpf"];
    if edition == Edition::Legacy {
        required.push("x64a.rpf");
    }
    for f in required {
        if !dir.join(f).is_file() {
            missing.push(f.to_string());
        }
    }
    missing
}

// ---------------- Steam ----------------

pub fn steam_root() -> Option<PathBuf> {
    let candidates = [
        reg_query_value("HKCU\\Software\\Valve\\Steam", "SteamPath"),
        reg_query_value("HKLM\\SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath"),
        reg_query_value("HKLM\\SOFTWARE\\Valve\\Steam", "InstallPath"),
        Some("C:\\Program Files (x86)\\Steam".to_string()),
    ];
    candidates
        .into_iter()
        .flatten()
        .map(|p| PathBuf::from(p.replace('/', "\\")))
        .find(|p| p.join("steamapps").is_dir())
}

fn steam_libraries(root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![root.to_path_buf()];
    let vdf = root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = fs::read_to_string(vdf) {
        let re = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
        for cap in re.captures_iter(&text) {
            let p = PathBuf::from(cap[1].replace("\\\\", "\\"));
            if !libs
                .iter()
                .any(|l| same_path(&l.to_string_lossy(), &p.to_string_lossy()))
            {
                libs.push(p);
            }
        }
    }
    libs
}

fn detect_steam(found: &mut Vec<Found>) {
    let Some(root) = steam_root() else { return };
    let re = Regex::new(r#""installdir"\s+"([^"]+)""#).unwrap();
    for lib in steam_libraries(&root) {
        for edition in Edition::ALL {
            let acf = lib
                .join("steamapps")
                .join(format!("appmanifest_{}.acf", edition.steam_appid()));
            let Ok(text) = fs::read_to_string(&acf) else {
                continue;
            };
            let Some(cap) = re.captures(&text) else {
                continue;
            };
            let dir = lib.join("steamapps").join("common").join(&cap[1]);
            if edition_of(&dir) == Some(edition) {
                push_unique(
                    found,
                    edition,
                    GameInstall {
                        path: dir.to_string_lossy().to_string(),
                        source: Source::Steam,
                        epic_id: None,
                        auto: true,
                    },
                );
            }
        }
    }
}

// ---------------- Epic Games ----------------

#[derive(Debug, Clone)]
pub struct EpicItem {
    pub location: String,
    pub launch_id: String,
}

pub fn epic_items() -> Vec<EpicItem> {
    let program_data = std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".into());
    let dir = PathBuf::from(program_data)
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    let mut items = vec![];
    let Ok(rd) = fs::read_dir(dir) else {
        return items;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("item") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&p) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let location = s("InstallLocation");
        if location.is_empty() {
            continue;
        }
        let launch_id = format!(
            "{}:{}:{}",
            s("CatalogNamespace"),
            s("CatalogItemId"),
            s("AppName")
        );
        items.push(EpicItem {
            location,
            launch_id,
        });
    }
    items
}

fn detect_epic(found: &mut Vec<Found>) {
    for item in epic_items() {
        let dir = PathBuf::from(&item.location);
        if let Some(edition) = edition_of(&dir) {
            push_unique(
                found,
                edition,
                GameInstall {
                    path: item.location.clone(),
                    source: Source::Epic,
                    epic_id: Some(item.launch_id.clone()),
                    auto: true,
                },
            );
        }
    }
}

// ---------------- Rockstar Games Launcher ----------------

fn detect_rockstar(found: &mut Vec<Found>) {
    let mut dirs: Vec<String> = vec![];
    for base in [
        "HKLM\\SOFTWARE\\WOW6432Node\\Rockstar Games",
        "HKLM\\SOFTWARE\\Rockstar Games",
    ] {
        for sub in reg_subkeys(base) {
            if let Some(p) = reg_query_value(&sub, "InstallFolder") {
                dirs.push(p);
            }
        }
    }
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    for name in ["Grand Theft Auto V", "Grand Theft Auto V Enhanced"] {
        dirs.push(format!("{pf}\\Rockstar Games\\{name}"));
    }
    for d in dirs {
        let dir = PathBuf::from(&d);
        if let Some(edition) = edition_of(&dir) {
            // Если та же папка уже найдена через Steam/Epic — это не Rockstar-версия.
            if found.iter().any(|f| same_path(&f.install.path, &d)) {
                continue;
            }
            push_unique(
                found,
                edition,
                GameInstall {
                    path: d,
                    source: Source::Rockstar,
                    epic_id: None,
                    auto: true,
                },
            );
        }
    }
}

pub fn rockstar_launcher_exe() -> Option<PathBuf> {
    let mut cands = vec![];
    if let Some(p) = reg_query_value(
        "HKLM\\SOFTWARE\\WOW6432Node\\Rockstar Games\\Launcher",
        "InstallFolder",
    ) {
        cands.push(PathBuf::from(p).join("Launcher.exe"));
    }
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    cands.push(
        PathBuf::from(pf)
            .join("Rockstar Games")
            .join("Launcher")
            .join("Launcher.exe"),
    );
    cands.into_iter().find(|p| p.is_file())
}

fn push_unique(found: &mut Vec<Found>, edition: Edition, install: GameInstall) {
    if found
        .iter()
        .any(|f| f.edition == edition && same_path(&f.install.path, &install.path))
    {
        return;
    }
    found.push(Found { edition, install });
}

/// Полный автопоиск.
pub fn detect_all() -> Vec<Found> {
    let mut found = vec![];
    detect_steam(&mut found);
    detect_epic(&mut found);
    detect_rockstar(&mut found);
    found
}

/// Определяет источник (клиент) для вручную выбранной папки.
pub fn classify_folder(dir: &Path) -> Result<(Edition, GameInstall), String> {
    let edition = edition_of(dir).ok_or_else(|| {
        "В этой папке нет GTA5.exe или GTA5_Enhanced.exe. Выберите корневую папку игры.".to_string()
    })?;
    let path = dir.to_string_lossy().to_string();
    let lower = path.to_lowercase();
    if lower.contains("steamapps") {
        return Ok((
            edition,
            GameInstall {
                path,
                source: Source::Steam,
                epic_id: None,
                auto: false,
            },
        ));
    }
    if let Some(item) = epic_items()
        .into_iter()
        .find(|i| same_path(&i.location, &path))
    {
        return Ok((
            edition,
            GameInstall {
                path,
                source: Source::Epic,
                epic_id: Some(item.launch_id),
                auto: false,
            },
        ));
    }
    let source = if dir.join("PlayGTAV.exe").is_file() {
        Source::Rockstar
    } else {
        Source::Unknown
    };
    Ok((
        edition,
        GameInstall {
            path,
            source,
            epic_id: None,
            auto: false,
        },
    ))
}

// ---------------- Версия файла ----------------

type VersionMap = HashMap<String, (SystemTime, Option<String>)>;
static VERSION_CACHE: OnceLock<Mutex<VersionMap>> = OnceLock::new();

/// Версия из ресурсов exe (VS_FIXEDFILEINFO). Если прочитать нельзя — `None`.
pub fn exe_version(exe: &Path) -> Option<String> {
    let key = exe.to_string_lossy().to_string();
    let mtime = fs::metadata(exe).and_then(|m| m.modified()).ok()?;
    let cache = VERSION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((t, v)) = cache.lock().ok()?.get(&key) {
        if *t == mtime {
            return v.clone();
        }
    }
    let v = read_pe_version(exe);
    if let Ok(mut c) = cache.lock() {
        c.insert(key, (mtime, v.clone()));
    }
    v
}

fn read_pe_version(exe: &Path) -> Option<String> {
    use pelite::pe64::{Pe, PeFile};
    let map = pelite::FileMap::open(exe).ok()?;
    let file = PeFile::from_bytes(&map).ok()?;
    let resources = file.resources().ok()?;
    let info = resources.version_info().ok()?;
    let fixed = info.fixed()?;
    let v = fixed.dwFileVersion;
    Some(format!("{}.{}.{}.{}", v.Major, v.Minor, v.Patch, v.Build))
}
