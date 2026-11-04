//! Менеджер модов для одиночного режима.
//!
//! Как это устроено:
//! * файлы мода хранятся в `%APPDATA%\…\mods\<id>\files\` в той же структуре, что и в папке игры;
//! * «Включить» — копирует файлы в папку игры. Если файл игры заменяется, оригинал
//!   сначала сохраняется в `backups\<редакция>\…`;
//! * «Отключить» — удаляет файлы мода из папки игры и возвращает оригиналы из резервной копии;
//! * два мода, которые пишут один и тот же файл, считаются конфликтующими.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::config::{AppState, Edition, ModEntry};
use crate::launch::{ensure_not_running, game_dir};
use crate::util::{copy_file_mkdirs, join_rel, now_iso, sanitize_rel, CmdResult, Ctx};

/// Папки игры: если архив начинается с них, корневую папку не срезаем.
const GAME_DIRS: &[&str] = &[
    "scripts",
    "mods",
    "update",
    "x64",
    "plugins",
    "menyoostuff",
    "lspdfr",
    "enbseries",
    "reshade-shaders",
];
const PROTECTED_FILES: &[&str] = &[
    "gta5.exe",
    "gta5_enhanced.exe",
    "gta5 enhanced.exe",
    "playgtav.exe",
    "gtavlauncher.exe",
];
const ASI_LOADERS: &[&str] = &["dinput8.dll", "version.dll", "xinput1_4.dll", "winmm.dll"];

#[derive(Clone)]
enum Origin {
    Zip(usize),
    Fs(PathBuf),
}

struct Entry {
    rel: String,
    size: u64,
    origin: Origin,
}

struct Collected {
    kind: &'static str,
    entries: Vec<Entry>,
    meta: Option<serde_json::Value>,
}

fn is_junk(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    lower.starts_with("__macosx/")
        || name == ".ds_store"
        || name == "thumbs.db"
        || name == "desktop.ini"
        || name.ends_with(".url")
}

fn collect(source: &Path) -> CmdResult<Collected> {
    if !source.exists() {
        return Err("Файл или папка не найдены.".into());
    }
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if source.is_file() {
        match ext.as_str() {
            "oiv" => return Err("Пакеты .oiv устанавливаются только через OpenIV (Package Installer). Лаунчер их не распаковывает.".into()),
            "rar" | "7z" => return Err("Архивы .rar и .7z не поддерживаются. Распакуйте архив и выберите папку мода или упакуйте его в .zip.".into()),
            _ => {}
        }
    }

    let mut entries = vec![];
    let mut meta = None;
    let kind;

    if source.is_dir() {
        kind = "folder";
        for e in walkdir::WalkDir::new(source).into_iter().flatten() {
            if !e.file_type().is_file() {
                continue;
            }
            let rel = e
                .path()
                .strip_prefix(source)
                .unwrap_or(e.path())
                .to_string_lossy()
                .to_string();
            let Some(rel) = sanitize_rel(&rel) else {
                continue;
            };
            if is_junk(&rel) {
                continue;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            entries.push(Entry {
                rel,
                size,
                origin: Origin::Fs(e.path().to_path_buf()),
            });
        }
    } else if ext == "zip" {
        kind = "zip";
        let f = fs::File::open(source).ctx("Не удалось открыть архив")?;
        let mut zip = zip::ZipArchive::new(f).ctx("Архив повреждён или не является .zip")?;
        for i in 0..zip.len() {
            let file = zip.by_index(i).ctx("Ошибка чтения архива")?;
            if file.is_dir() {
                continue;
            }
            let Some(name) = file.enclosed_name() else {
                continue;
            };
            let Some(rel) = sanitize_rel(&name.to_string_lossy()) else {
                continue;
            };
            if is_junk(&rel) {
                continue;
            }
            entries.push(Entry {
                rel,
                size: file.size(),
                origin: Origin::Zip(i),
            });
        }
    } else {
        kind = "file";
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let size = fs::metadata(source).map(|m| m.len()).unwrap_or(0);
        entries.push(Entry {
            rel: name,
            size,
            origin: Origin::Fs(source.to_path_buf()),
        });
    }

    if entries.is_empty() {
        return Err("Внутри нет файлов для установки.".into());
    }

    strip_common_root(&mut entries);

    // mod.json в корне — описание мода (name, version, description, category).
    if let Some(pos) = entries
        .iter()
        .position(|e| e.rel.eq_ignore_ascii_case("mod.json"))
    {
        let e = entries.remove(pos);
        if let Ok(text) = read_entry(source, &e) {
            meta = serde_json::from_slice(&text).ok();
        }
    }
    Ok(Collected {
        kind,
        entries,
        meta,
    })
}

fn strip_common_root(entries: &mut [Entry]) {
    loop {
        let first: Option<String> = entries
            .first()
            .and_then(|e| e.rel.split_once('/').map(|(a, _)| a.to_string()));
        let Some(first) = first else { return };
        let all_share = entries.iter().all(|e| {
            e.rel
                .split_once('/')
                .map(|(a, _)| a == first)
                .unwrap_or(false)
        });
        if !all_share || GAME_DIRS.contains(&first.to_lowercase().as_str()) {
            return;
        }
        for e in entries.iter_mut() {
            e.rel = e.rel[first.len() + 1..].to_string();
        }
    }
}

fn read_entry(source: &Path, e: &Entry) -> CmdResult<Vec<u8>> {
    match &e.origin {
        Origin::Fs(p) => fs::read(p).ctx("Не удалось прочитать файл"),
        Origin::Zip(i) => {
            let f = fs::File::open(source).ctx("Не удалось открыть архив")?;
            let mut zip = zip::ZipArchive::new(f).ctx("Ошибка архива")?;
            let mut file = zip.by_index(*i).ctx("Ошибка архива")?;
            let mut buf = Vec::with_capacity(e.size as usize);
            file.read_to_end(&mut buf).ctx("Ошибка распаковки")?;
            Ok(buf)
        }
    }
}

fn suggest_target(entries: &[Entry]) -> String {
    let top_level: Vec<&str> = entries
        .iter()
        .map(|e| e.rel.as_str())
        .filter(|r| !r.contains('/'))
        .collect();
    let has_dirs = entries.iter().any(|e| e.rel.contains('/'));
    let lower: Vec<String> = top_level.iter().map(|s| s.to_lowercase()).collect();
    let root_markers = lower.iter().any(|n| {
        n.ends_with(".asi")
            || ASI_LOADERS.contains(&n.as_str())
            || n == "scripthookv.dll"
            || n.starts_with("scripthookvdotnet")
    });
    if root_markers || has_dirs {
        return String::new();
    }
    let script_like = !lower.is_empty()
        && lower.iter().all(|n| {
            n.ends_with(".dll")
                || n.ends_with(".cs")
                || n.ends_with(".vb")
                || n.ends_with(".ini")
                || n.ends_with(".xml")
                || n.ends_with(".pdb")
        })
        && lower
            .iter()
            .any(|n| n.ends_with(".dll") || n.ends_with(".cs") || n.ends_with(".vb"));
    if script_like {
        "scripts".into()
    } else {
        String::new()
    }
}

fn guess_category(name: &str, entries: &[Entry]) -> String {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&[
        "car",
        "vehicle",
        "bike",
        "plane",
        "heli",
        "авто",
        "машин",
        "транспорт",
    ]) {
        "vehicles"
    } else if has(&[
        "reshade",
        "enb",
        "graphic",
        "visual",
        "texture",
        "naturalvision",
        "quantv",
        "графи",
    ]) {
        "graphics"
    } else if has(&[
        "ped",
        "skin",
        "character",
        "outfit",
        "cloth",
        "персонаж",
        "одежд",
    ]) {
        "characters"
    } else if has(&["map", "ymap", "mlo", "interior", "карт"]) {
        "maps"
    } else if has(&["trainer", "menu", "gameplay", "realism", "геймплей"]) {
        "gameplay"
    } else if entries.iter().any(|e| {
        let r = e.rel.to_lowercase();
        r.ends_with(".asi") || r.ends_with(".cs") || r.starts_with("scripts/")
    }) {
        "scripts"
    } else {
        "other"
    }
    .into()
}

fn validate_files(rels: &[String]) -> Result<(), String> {
    for r in rels {
        let lower = r.to_lowercase();
        let name = lower.rsplit('/').next().unwrap_or(&lower);
        if !lower.contains('/') && PROTECTED_FILES.contains(&name) {
            return Err(format!("Мод пытается заменить исполняемый файл игры ({r}). Такие моды лаунчер не устанавливает."));
        }
        if lower.ends_with(".rpf") && !lower.starts_with("mods/") {
            return Err(format!(
                "Мод заменяет оригинальный архив игры ({r}). Замена .rpf допускается только в папке mods/ — установите такой мод через OpenIV в папку mods."
            ));
        }
    }
    Ok(())
}

fn with_target(target: &str, rel: &str) -> Option<String> {
    let target = target.trim().trim_matches(['/', '\\']);
    if target.is_empty() {
        sanitize_rel(rel)
    } else {
        sanitize_rel(&format!("{target}/{rel}"))
    }
}

#[derive(Serialize)]
pub struct ModInspect {
    pub kind: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub category: String,
    pub target_subdir: String,
    pub files: Vec<String>,
    pub file_count: usize,
    pub total_size: u64,
    pub warnings: Vec<String>,
}

pub fn inspect(source: &Path, edition: Edition) -> CmdResult<ModInspect> {
    let c = collect(source)?;
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Мод".into());
    let m = |k: &str| {
        c.meta
            .as_ref()
            .and_then(|v| v.get(k))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };
    let name = m("name").unwrap_or(stem);
    let target = m("target").unwrap_or_else(|| suggest_target(&c.entries));
    let category = m("category").unwrap_or_else(|| guess_category(&name, &c.entries));
    let mut warnings = vec![];
    let rels: Vec<String> = c
        .entries
        .iter()
        .filter_map(|e| with_target(&target, &e.rel))
        .collect();
    if let Err(e) = validate_files(&rels) {
        warnings.push(e);
    }
    let lower: Vec<String> = rels.iter().map(|r| r.to_lowercase()).collect();
    if edition == Edition::Enhanced
        && lower
            .iter()
            .any(|r| r.ends_with(".asi") || r == "scripthookv.dll")
    {
        warnings.push("Мод использует ASI/ScriptHookV. Многие такие моды сделаны только для Legacy — убедитесь, что версия мода поддерживает Enhanced.".into());
    }
    if lower.iter().any(|r| r.ends_with(".exe")) {
        warnings.push(
            "В моде есть .exe-файлы. Лаунчер их не запускает, но проверьте источник мода.".into(),
        );
    }
    Ok(ModInspect {
        kind: c.kind.into(),
        name,
        version: m("version").unwrap_or_default(),
        description: m("description").unwrap_or_default(),
        category,
        target_subdir: target,
        file_count: rels.len(),
        total_size: c.entries.iter().map(|e| e.size).sum(),
        files: rels.into_iter().take(300).collect(),
        warnings,
    })
}

#[derive(Deserialize)]
pub struct InstallRequest {
    pub edition: Edition,
    pub source_path: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub category: String,
    pub target_subdir: String,
    pub enable: bool,
}

#[derive(Serialize, Default)]
pub struct Conflict {
    pub file: String,
    pub mod_id: String,
    pub mod_name: String,
}

#[derive(Serialize, Default)]
pub struct EnableResult {
    pub ok: bool,
    pub conflicts: Vec<Conflict>,
    pub mod_id: String,
}

pub fn install(state: &AppState, req: InstallRequest) -> CmdResult<EnableResult> {
    let source = PathBuf::from(&req.source_path);
    let c = collect(&source)?;
    let mut files = vec![];
    for e in &c.entries {
        let rel = with_target(&req.target_subdir, &e.rel).ok_or("Недопустимая папка установки")?;
        files.push((rel, e));
    }
    let rels: Vec<String> = files.iter().map(|(r, _)| r.clone()).collect();
    validate_files(&rels)?;
    let unique: HashSet<String> = rels.iter().map(|r| r.to_lowercase()).collect();
    if unique.len() != rels.len() {
        return Err(
            "В моде есть файлы с одинаковыми именами (различаются только регистром).".into(),
        );
    }

    let id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
    let store = state.path("mods").join(&id).join("files");
    fs::create_dir_all(&store).ctx("Не удалось создать папку мода")?;

    let result = (|| -> CmdResult<u64> {
        let mut total = 0;
        let mut zip = if c.kind == "zip" {
            Some(zip::ZipArchive::new(fs::File::open(&source).ctx("Архив")?).ctx("Архив")?)
        } else {
            None
        };
        for (rel, e) in &files {
            let dst = join_rel(&store, rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).ctx("Ошибка создания папки")?;
            }
            match &e.origin {
                Origin::Fs(p) => {
                    fs::copy(p, &dst).ctx(&format!("Ошибка копирования {rel}"))?;
                }
                Origin::Zip(i) => {
                    let z = zip.as_mut().ok_or("Архив")?;
                    let mut zf = z.by_index(*i).ctx("Ошибка архива")?;
                    let mut out = fs::File::create(&dst).ctx("Ошибка записи")?;
                    std::io::copy(&mut zf, &mut out).ctx(&format!("Ошибка распаковки {rel}"))?;
                }
            }
            total += e.size;
        }
        Ok(total)
    })();

    let size = match result {
        Ok(s) => s,
        Err(err) => {
            let _ = fs::remove_dir_all(state.path("mods").join(&id));
            return Err(err);
        }
    };

    let entry = ModEntry {
        id: id.clone(),
        name: if req.name.trim().is_empty() {
            "Без названия".into()
        } else {
            req.name.trim().into()
        },
        version: req.version.trim().into(),
        description: req.description.trim().into(),
        category: req.category,
        edition: req.edition,
        enabled: false,
        files: rels,
        size,
        installed_at: now_iso(),
        source_name: source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
    };
    state.update(|c| c.mods.push(entry))?;

    if req.enable {
        let mut r = enable(state, &id, false)?;
        r.mod_id = id;
        return Ok(r);
    }
    Ok(EnableResult {
        ok: true,
        conflicts: vec![],
        mod_id: id,
    })
}

fn owners(mods: &[ModEntry], edition: Edition, except: &str) -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    for m in mods
        .iter()
        .filter(|m| m.enabled && m.edition == edition && m.id != except)
    {
        for f in &m.files {
            map.insert(f.to_lowercase(), (m.id.clone(), m.name.clone()));
        }
    }
    map
}

pub fn enable(state: &AppState, id: &str, force: bool) -> CmdResult<EnableResult> {
    ensure_not_running()?;
    let cfg = state.snapshot();
    let m = cfg
        .mods
        .iter()
        .find(|m| m.id == id)
        .cloned()
        .ok_or("Мод не найден")?;
    if m.enabled {
        return Ok(EnableResult {
            ok: true,
            ..Default::default()
        });
    }
    let game = game_dir(state, m.edition)?;
    let own = owners(&cfg.mods, m.edition, id);
    let conflicts: Vec<Conflict> = m
        .files
        .iter()
        .filter_map(|f| {
            own.get(&f.to_lowercase()).map(|(mid, name)| Conflict {
                file: f.clone(),
                mod_id: mid.clone(),
                mod_name: name.clone(),
            })
        })
        .collect();
    if !conflicts.is_empty() {
        if !force {
            return Ok(EnableResult {
                ok: false,
                conflicts,
                mod_id: id.into(),
            });
        }
        let ids: HashSet<String> = conflicts.iter().map(|c| c.mod_id.clone()).collect();
        for other in ids {
            disable(state, &other)?;
        }
    }

    let store = state.path("mods").join(id).join("files");
    let backup_root = state.path("backups").join(edition_key(m.edition));
    let mut copied: Vec<String> = vec![];
    for rel in &m.files {
        let src = join_rel(&store, rel);
        let dst = join_rel(&game, rel);
        if !src.is_file() {
            rollback(&game, &backup_root, &copied);
            return Err(format!(
                "Файл мода пропал из хранилища лаунчера: {rel}. Переустановите мод."
            ));
        }
        if dst.exists() {
            let bak = join_rel(&backup_root, rel);
            if !bak.exists() {
                if let Err(e) = copy_file_mkdirs(&dst, &bak) {
                    rollback(&game, &backup_root, &copied);
                    return Err(format!("Не удалось сделать резервную копию {rel}: {e}"));
                }
            }
        }
        if let Err(e) = copy_file_mkdirs(&src, &dst) {
            rollback(&game, &backup_root, &copied);
            return Err(format!(
                "Не удалось скопировать {rel}: {e}. Если игра установлена в Program Files, запустите лаунчер от имени администратора."
            ));
        }
        copied.push(rel.clone());
    }
    state.update(|c| {
        if let Some(x) = c.mods.iter_mut().find(|x| x.id == id) {
            x.enabled = true;
        }
    })?;
    Ok(EnableResult {
        ok: true,
        conflicts: vec![],
        mod_id: id.into(),
    })
}

fn rollback(game: &Path, backup_root: &Path, files: &[String]) {
    for rel in files {
        restore_one(game, backup_root, rel);
    }
}

fn restore_one(game: &Path, backup_root: &Path, rel: &str) {
    let dst = join_rel(game, rel);
    let bak = join_rel(backup_root, rel);
    let _ = fs::remove_file(&dst);
    if bak.is_file() && copy_file_mkdirs(&bak, &dst).is_ok() {
        let _ = fs::remove_file(&bak);
    }
}

pub fn edition_key(e: Edition) -> &'static str {
    match e {
        Edition::Legacy => "legacy",
        Edition::Enhanced => "enhanced",
    }
}

pub fn disable(state: &AppState, id: &str) -> CmdResult<()> {
    ensure_not_running()?;
    let cfg = state.snapshot();
    let m = cfg
        .mods
        .iter()
        .find(|m| m.id == id)
        .cloned()
        .ok_or("Мод не найден")?;
    if !m.enabled {
        return Ok(());
    }
    let game = game_dir(state, m.edition)?;
    let backup_root = state.path("backups").join(edition_key(m.edition));
    for rel in &m.files {
        restore_one(&game, &backup_root, rel);
    }
    state.update(|c| {
        if let Some(x) = c.mods.iter_mut().find(|x| x.id == id) {
            x.enabled = false;
        }
    })?;
    Ok(())
}

pub fn remove(state: &AppState, id: &str) -> CmdResult<()> {
    let cfg = state.snapshot();
    let m = cfg
        .mods
        .iter()
        .find(|m| m.id == id)
        .cloned()
        .ok_or("Мод не найден")?;
    if m.enabled {
        disable(state, id)?;
    }
    let _ = fs::remove_dir_all(state.path("mods").join(id));
    state.update(|c| c.mods.retain(|x| x.id != id))?;
    Ok(())
}

pub fn disable_all(state: &AppState, edition: Edition) -> CmdResult<usize> {
    let ids: Vec<String> = state
        .snapshot()
        .mods
        .iter()
        .filter(|m| m.enabled && m.edition == edition)
        .map(|m| m.id.clone())
        .collect();
    for id in &ids {
        disable(state, id)?;
    }
    Ok(ids.len())
}

/// Применить набор модов профиля: всё лишнее отключить, нужное включить.
pub fn apply_set(state: &AppState, edition: Edition, wanted: &[String]) -> CmdResult<Vec<String>> {
    let mut notes = vec![];
    let cfg = state.snapshot();
    for m in cfg
        .mods
        .iter()
        .filter(|m| m.edition == edition && m.enabled && !wanted.contains(&m.id))
    {
        disable(state, &m.id)?;
    }
    for id in wanted {
        let Some(m) = cfg
            .mods
            .iter()
            .find(|m| &m.id == id && m.edition == edition)
        else {
            continue;
        };
        let r = enable(state, id, false)?;
        if !r.ok {
            notes.push(format!(
                "«{}» не включён: конфликт с «{}»",
                m.name, r.conflicts[0].mod_name
            ));
        }
    }
    Ok(notes)
}

#[derive(Serialize)]
pub struct ExternalItem {
    pub path: String,
    pub kind: String,
    pub size: u64,
}

/// Модификации в папке игры, которые поставлены не через лаунчер.
pub fn scan_external(state: &AppState, edition: Edition) -> CmdResult<Vec<ExternalItem>> {
    let game = game_dir(state, edition)?;
    let cfg = state.snapshot();
    let managed: HashSet<String> = cfg
        .mods
        .iter()
        .filter(|m| m.enabled && m.edition == edition)
        .flat_map(|m| m.files.iter().map(|f| f.to_lowercase()))
        .collect();
    let mut out = vec![];
    let mut push = |rel: String, kind: &str, size: u64| {
        if !managed.contains(&rel.to_lowercase()) {
            out.push(ExternalItem {
                path: rel,
                kind: kind.into(),
                size,
            });
        }
    };
    if let Ok(rd) = fs::read_dir(&game) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            let is_file = e.path().is_file();
            let kind = if is_file && lower.ends_with(".asi") {
                Some("ASI-плагин")
            } else if is_file && ASI_LOADERS.contains(&lower.as_str()) {
                Some("ASI Loader")
            } else if is_file && lower == "scripthookv.dll" {
                Some("Script Hook V")
            } else if is_file
                && (lower == "dxgi.dll"
                    || lower == "d3d11.dll"
                    || lower == "d3d12.dll"
                    || lower.starts_with("reshade"))
            {
                Some("Графический инжектор (ReShade/ENB)")
            } else if !is_file && (lower == "enbseries" || lower == "reshade-shaders") {
                Some("Графика (папка)")
            } else if !is_file && lower == "mods" {
                Some("Папка mods (OpenIV)")
            } else {
                None
            };
            if let Some(k) = kind {
                push(name, k, size);
            }
        }
    }
    let scripts = game.join("scripts");
    if let Ok(rd) = fs::read_dir(&scripts) {
        for e in rd.flatten() {
            if !e.path().is_file() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if lower.ends_with(".dll")
                || lower.ends_with(".cs")
                || lower.ends_with(".vb")
                || lower.ends_with(".lua")
            {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                push(format!("scripts/{name}"), "Скрипт", size);
            }
        }
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct ModWarning {
    pub level: String,
    pub text: String,
}

/// Проверка совместимости и конфликтов (по реальному содержимому папки игры).
pub fn check(state: &AppState, edition: Edition) -> CmdResult<Vec<ModWarning>> {
    let game = game_dir(state, edition)?;
    let cfg = state.snapshot();
    let mut w = vec![];
    let mut add = |level: &str, text: String| {
        w.push(ModWarning {
            level: level.into(),
            text,
        })
    };

    // Конфликты между модами лаунчера (на случай ручных правок).
    let mut seen: HashMap<String, String> = HashMap::new();
    for m in cfg
        .mods
        .iter()
        .filter(|m| m.enabled && m.edition == edition)
    {
        for f in &m.files {
            if let Some(prev) = seen.insert(f.to_lowercase(), m.name.clone()) {
                add(
                    "error",
                    format!("Конфликт: «{}» и «{}» заменяют один файл {f}", prev, m.name),
                );
            }
        }
    }
    let exists = |rel: &str| game.join(rel).exists();
    let root_files: Vec<String> = fs::read_dir(&game)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let asi_count = root_files.iter().filter(|n| n.ends_with(".asi")).count();
    let loaders: Vec<&str> = ASI_LOADERS
        .iter()
        .copied()
        .filter(|l| root_files.iter().any(|n| n == l))
        .collect();
    let has_shv = root_files.iter().any(|n| n == "scripthookv.dll");
    let has_shvdn = root_files
        .iter()
        .any(|n| n.starts_with("scripthookvdotnet") && n.ends_with(".asi"));
    let scripts_count = fs::read_dir(game.join("scripts"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    n.ends_with(".dll") || n.ends_with(".cs") || n.ends_with(".vb")
                })
                .count()
        })
        .unwrap_or(0);

    if asi_count > 0 && loaders.is_empty() {
        add("warn", format!("Найдено ASI-плагинов: {asi_count}, но нет ASI Loader (dinput8.dll). Они не загрузятся."));
    }
    if loaders.len() > 1 {
        add(
            "warn",
            format!(
                "Похоже, установлено несколько ASI Loader: {}. Обычно нужен только один.",
                loaders.join(", ")
            ),
        );
    }
    if has_shvdn && !has_shv {
        add(
            "error",
            "ScriptHookVDotNet установлен без ScriptHookV.dll — скрипты не будут работать.".into(),
        );
    }
    if scripts_count > 0 && !has_shvdn {
        add(
            "warn",
            format!(
                "В папке scripts {scripts_count} скрипт(ов), но не найден ScriptHookVDotNet*.asi."
            ),
        );
    }
    if edition == Edition::Enhanced && (has_shv || asi_count > 0) {
        add("warn", "Для GTA V Enhanced подходят только версии ScriptHookV/ASI-модов, выпущенные для Enhanced. Моды для Legacy могут вызывать вылеты.".into());
    }
    if exists("mods")
        && !root_files
            .iter()
            .any(|n| n == "openiv.asi" || n == "openrpf.asi")
    {
        add(
            "warn",
            "Есть папка mods, но нет OpenIV.asi/OpenRPF.asi — файлы из mods игра не загрузит."
                .into(),
        );
    }
    if has_shv {
        add("info", "После обновления игры ScriptHookV перестаёт работать, пока автор не выпустит новую версию. Если игра вылетает сразу после обновления — отключите моды.".into());
    }
    Ok(w)
}

#[derive(Serialize)]
pub struct Preflight {
    pub managed_enabled: usize,
    pub external: usize,
}

pub fn preflight(state: &AppState, edition: Edition) -> Preflight {
    let managed_enabled = state
        .snapshot()
        .mods
        .iter()
        .filter(|m| m.enabled && m.edition == edition)
        .count();
    let external = scan_external(state, edition)
        .map(|v| v.iter().filter(|i| i.kind != "Папка mods (OpenIV)").count())
        .unwrap_or(0);
    Preflight {
        managed_enabled,
        external,
    }
}

pub fn update_meta(
    state: &AppState,
    id: &str,
    name: String,
    version: String,
    description: String,
    category: String,
) -> CmdResult<()> {
    state.update(|c| {
        if let Some(m) = c.mods.iter_mut().find(|m| m.id == id) {
            if !name.trim().is_empty() {
                m.name = name.trim().into();
            }
            m.version = version.trim().into();
            m.description = description.trim().into();
            m.category = category;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(rel: &str) -> Entry {
        Entry {
            rel: rel.into(),
            size: 1,
            origin: Origin::Fs(PathBuf::new()),
        }
    }

    #[test]
    fn strips_wrapper_folder() {
        let mut v = vec![e("MyMod/scripts/a.dll"), e("MyMod/readme.txt")];
        strip_common_root(&mut v);
        assert_eq!(v[0].rel, "scripts/a.dll");
        let mut v = vec![e("scripts/a.dll"), e("scripts/b.dll")];
        strip_common_root(&mut v);
        assert_eq!(v[0].rel, "scripts/a.dll");
    }

    #[test]
    fn target_suggestion() {
        assert_eq!(suggest_target(&[e("MyScript.dll")]), "scripts");
        assert_eq!(
            suggest_target(&[e("ScriptHookV.dll"), e("dinput8.dll")]),
            ""
        );
        assert_eq!(suggest_target(&[e("plugin.asi"), e("plugin.ini")]), "");
    }

    #[test]
    fn validation() {
        assert!(validate_files(&["GTA5.exe".into()]).is_err());
        assert!(validate_files(&["x64e.rpf".into()]).is_err());
        assert!(validate_files(&["mods/x64e.rpf".into()]).is_ok());
        assert!(validate_files(&["scripts/a.dll".into()]).is_ok());
    }

    #[test]
    fn enable_disable_roundtrip() {
        use crate::config::{AppState, GameInstall, Source};
        let tmp = std::env::temp_dir().join(format!("gtavl-test-{}", uuid::Uuid::new_v4()));
        let game = tmp.join("game");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("GTA5.exe"), vec![0u8; 1_100_000]).unwrap();
        fs::write(game.join("dinput8.dll"), b"original").unwrap();
        let src = tmp.join("src");
        fs::create_dir_all(src.join("Pack")).unwrap();
        fs::write(src.join("Pack").join("dinput8.dll"), b"modded").unwrap();
        fs::write(src.join("Pack").join("cool.asi"), b"asi").unwrap();

        let state = AppState::load(tmp.join("data"));
        state
            .update(|c| {
                c.installs.legacy = Some(GameInstall {
                    path: game.to_string_lossy().into(),
                    source: Source::Rockstar,
                    epic_id: None,
                    auto: false,
                })
            })
            .unwrap();
        let info = inspect(&src, Edition::Legacy).unwrap();
        assert_eq!(info.target_subdir, "");
        let r = install(
            &state,
            InstallRequest {
                edition: Edition::Legacy,
                source_path: src.to_string_lossy().into(),
                name: "Test".into(),
                version: "1".into(),
                description: String::new(),
                category: "scripts".into(),
                target_subdir: String::new(),
                enable: true,
            },
        )
        .unwrap();
        assert!(r.ok);
        assert_eq!(fs::read(game.join("dinput8.dll")).unwrap(), b"modded");
        assert!(game.join("cool.asi").exists());

        // второй мод с тем же файлом — конфликт
        let src2 = tmp.join("src2");
        fs::create_dir_all(&src2).unwrap();
        fs::write(src2.join("dinput8.dll"), b"other").unwrap();
        let r2 = install(
            &state,
            InstallRequest {
                edition: Edition::Legacy,
                source_path: src2.join("dinput8.dll").to_string_lossy().into(),
                name: "Other".into(),
                version: String::new(),
                description: String::new(),
                category: "other".into(),
                target_subdir: String::new(),
                enable: true,
            },
        )
        .unwrap();
        assert!(!r2.ok);
        assert_eq!(r2.conflicts.len(), 1);

        disable(&state, &r.mod_id).unwrap();
        assert_eq!(fs::read(game.join("dinput8.dll")).unwrap(), b"original");
        assert!(!game.join("cool.asi").exists());
        remove(&state, &r.mod_id).unwrap();
        assert!(state.snapshot().mods.iter().all(|m| m.id != r.mod_id));
        let _ = fs::remove_dir_all(&tmp);
    }
}
