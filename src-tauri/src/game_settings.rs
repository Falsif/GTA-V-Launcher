//! Настройки игры: «Документы\Rockstar Games\<GTA V | GTAV Enhanced>\settings.xml»,
//! commandline.txt в папке игры, профили настроек и резервные копии.
//!
//! Меняются только параметры, которые уже есть в settings.xml (его создаёт сама игра).

use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::config::{AppState, Edition};
use crate::launch::{ensure_not_running, game_dir};
use crate::mods::edition_key;
use crate::util::{copy_file_mkdirs, timestamp_slug, CmdResult, Ctx};

pub fn documents_dir(edition: Edition) -> CmdResult<PathBuf> {
    let docs = dirs::document_dir().ok_or("Не найдена папка «Документы»")?;
    Ok(docs.join("Rockstar Games").join(edition.documents_folder()))
}

pub fn settings_path(edition: Edition) -> CmdResult<PathBuf> {
    Ok(documents_dir(edition)?.join("settings.xml"))
}

#[derive(Serialize)]
pub struct GameSettings {
    pub path: String,
    pub exists: bool,
    /// Все параметры вида `<Name value="..."/>` из файла.
    pub values: BTreeMap<String, String>,
}

fn value_re() -> Regex {
    Regex::new(r#"<([A-Za-z_][A-Za-z0-9_]*)\s+value="([^"]*)""#).unwrap()
}

pub fn read(edition: Edition) -> CmdResult<GameSettings> {
    let path = settings_path(edition)?;
    let mut values = BTreeMap::new();
    let exists = path.is_file();
    if exists {
        let text = fs::read_to_string(&path).ctx("Не удалось прочитать settings.xml")?;
        for cap in value_re().captures_iter(&text) {
            values
                .entry(cap[1].to_string())
                .or_insert_with(|| cap[2].to_string());
        }
    }
    Ok(GameSettings {
        path: path.to_string_lossy().into(),
        exists,
        values,
    })
}

pub fn write(
    state: &AppState,
    edition: Edition,
    changes: BTreeMap<String, String>,
) -> CmdResult<usize> {
    ensure_not_running()?;
    let path = settings_path(edition)?;
    if !path.is_file() {
        return Err(
            "settings.xml ещё не создан. Запустите игру один раз — она создаст файл сама.".into(),
        );
    }
    let mut text = fs::read_to_string(&path).ctx("Не удалось прочитать settings.xml")?;
    let ok_value = Regex::new(r"^[-A-Za-z0-9_.]{1,32}$").unwrap();
    let mut applied = 0;
    for (key, value) in &changes {
        if !ok_value.is_match(value) {
            return Err(format!("Недопустимое значение для {key}: {value}"));
        }
        let re = Regex::new(&format!(r#"(<{}\s+value=")[^"]*(")"#, regex::escape(key))).unwrap();
        if !re.is_match(&text) {
            return Err(format!(
                "Параметр {key} не поддерживается этой версией игры."
            ));
        }
        text = re
            .replacen(&text, 1, |c: &regex::Captures| {
                format!("{}{}{}", &c[1], value, &c[2])
            })
            .into_owned();
        applied += 1;
    }
    backup_now(state, edition, "auto")?;
    fs::write(&path, text).ctx("Не удалось записать settings.xml")?;
    Ok(applied)
}

// ---------- commandline.txt ----------

pub fn read_commandline(state: &AppState, edition: Edition) -> CmdResult<String> {
    let p = game_dir(state, edition)?.join("commandline.txt");
    Ok(fs::read_to_string(p).unwrap_or_default())
}

pub fn write_commandline(state: &AppState, edition: Edition, text: String) -> CmdResult<()> {
    ensure_not_running()?;
    let p = game_dir(state, edition)?.join("commandline.txt");
    backup_now(state, edition, "auto")?;
    if text.trim().is_empty() {
        if p.exists() {
            fs::remove_file(&p).ctx("Не удалось удалить commandline.txt")?;
        }
        return Ok(());
    }
    fs::write(&p, text.trim().to_string() + "\n").ctx(
        "Не удалось записать commandline.txt (если игра в Program Files — запустите лаунчер от имени администратора)",
    )
}

// ---------- Профили настроек ----------

fn profiles_dir(state: &AppState, edition: Edition) -> PathBuf {
    state.path("settings_profiles").join(edition_key(edition))
}

fn safe_name(name: &str) -> CmdResult<String> {
    let n: String = name
        .trim()
        .chars()
        .filter(|c| !"\\/:*?\"<>|".contains(*c))
        .collect();
    if n.is_empty() || n.len() > 60 {
        return Err("Введите название профиля (до 60 символов).".into());
    }
    Ok(n)
}

pub fn list_profiles(state: &AppState, edition: Edition) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(profiles_dir(state, edition))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.strip_suffix(".xml").map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

pub fn save_profile(state: &AppState, edition: Edition, name: &str) -> CmdResult<()> {
    let src = settings_path(edition)?;
    if !src.is_file() {
        return Err("settings.xml не найден — сохранять нечего.".into());
    }
    let n = safe_name(name)?;
    copy_file_mkdirs(&src, &profiles_dir(state, edition).join(format!("{n}.xml")))
        .ctx("Не удалось сохранить профиль")
}

pub fn apply_profile(state: &AppState, edition: Edition, name: &str) -> CmdResult<()> {
    ensure_not_running()?;
    let n = safe_name(name)?;
    let src = profiles_dir(state, edition).join(format!("{n}.xml"));
    if !src.is_file() {
        return Err("Профиль не найден".into());
    }
    if settings_path(edition)?.is_file() {
        backup_now(state, edition, "auto")?;
    }
    copy_file_mkdirs(&src, &settings_path(edition)?).ctx("Не удалось применить профиль")
}

pub fn delete_profile(state: &AppState, edition: Edition, name: &str) -> CmdResult<()> {
    let n = safe_name(name)?;
    fs::remove_file(profiles_dir(state, edition).join(format!("{n}.xml")))
        .ctx("Не удалось удалить профиль")
}

// ---------- Резервные копии конфигурации ----------

#[derive(Serialize)]
pub struct BackupInfo {
    pub id: String,
    pub files: Vec<String>,
}

fn backups_dir(state: &AppState, edition: Edition) -> PathBuf {
    state.path("config_backups").join(edition_key(edition))
}

/// Копирует settings.xml и commandline.txt. Хранится не больше 30 автокопий.
pub fn backup_now(state: &AppState, edition: Edition, kind: &str) -> CmdResult<String> {
    let id = format!("{}_{kind}", timestamp_slug());
    let dir = backups_dir(state, edition).join(&id);
    let mut any = false;
    if let Ok(p) = settings_path(edition) {
        if p.is_file() {
            copy_file_mkdirs(&p, &dir.join("settings.xml")).ctx("Резервная копия settings.xml")?;
            any = true;
        }
    }
    if let Ok(g) = game_dir(state, edition) {
        let p = g.join("commandline.txt");
        if p.is_file() {
            copy_file_mkdirs(&p, &dir.join("commandline.txt"))
                .ctx("Резервная копия commandline.txt")?;
            any = true;
        }
    }
    if !any {
        return Err("Нет файлов конфигурации для копирования (settings.xml создаётся после первого запуска игры).".into());
    }
    prune_auto(state, edition);
    Ok(id)
}

fn prune_auto(state: &AppState, edition: Edition) {
    let mut autos: Vec<String> = list_backups(state, edition)
        .into_iter()
        .map(|b| b.id)
        .filter(|id| id.ends_with("_auto"))
        .collect();
    autos.sort();
    while autos.len() > 30 {
        let old = autos.remove(0);
        let _ = fs::remove_dir_all(backups_dir(state, edition).join(old));
    }
}

pub fn list_backups(state: &AppState, edition: Edition) -> Vec<BackupInfo> {
    let mut v: Vec<BackupInfo> = fs::read_dir(backups_dir(state, edition))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| BackupInfo {
                    id: e.file_name().to_string_lossy().into(),
                    files: fs::read_dir(e.path())
                        .map(|r| {
                            r.flatten()
                                .map(|f| f.file_name().to_string_lossy().into())
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.id.cmp(&a.id));
    v
}

fn check_id(id: &str) -> CmdResult<()> {
    if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
        return Err("Неверная резервная копия".into());
    }
    Ok(())
}

pub fn restore_backup(state: &AppState, edition: Edition, id: &str) -> CmdResult<()> {
    ensure_not_running()?;
    check_id(id)?;
    let dir = backups_dir(state, edition).join(id);
    let s = dir.join("settings.xml");
    let c = dir.join("commandline.txt");
    // Текущее состояние тоже сохраняем, чтобы восстановление можно было отменить.
    let _ = backup_now(state, edition, "auto");
    if s.is_file() {
        copy_file_mkdirs(&s, &settings_path(edition)?)
            .ctx("Не удалось восстановить settings.xml")?;
    }
    if c.is_file() {
        let g = game_dir(state, edition)?;
        copy_file_mkdirs(&c, &g.join("commandline.txt"))
            .ctx("Не удалось восстановить commandline.txt")?;
    }
    Ok(())
}

pub fn delete_backup(state: &AppState, edition: Edition, id: &str) -> CmdResult<()> {
    check_id(id)?;
    fs::remove_dir_all(backups_dir(state, edition).join(id)).ctx("Не удалось удалить")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regex_replace() {
        let text = r#"<video><Windowed value="0" /><VSync value="1" /></video>"#;
        let re = Regex::new(&format!(r#"(<{}\s+value=")[^"]*(")"#, "VSync")).unwrap();
        let out = re.replacen(text, 1, |c: &regex::Captures| {
            format!("{}{}{}", &c[1], "2", &c[2])
        });
        assert_eq!(
            out,
            r#"<video><Windowed value="0" /><VSync value="2" /></video>"#
        );
        let vals: Vec<_> = value_re()
            .captures_iter(text)
            .map(|c| c[1].to_string())
            .collect();
        assert_eq!(vals, vec!["Windowed", "VSync"]);
    }
}
