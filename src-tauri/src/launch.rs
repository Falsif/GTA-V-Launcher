//! Запуск игры через официальный клиент, проверка целостности и учёт времени игры.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::config::{AppState, Edition, GameInstall, Source};
use crate::detect;
use crate::util::{hidden_command, now_iso, CmdResult, Ctx};

#[derive(Serialize, Clone, Debug, Default)]
pub struct RunningStatus {
    pub running: bool,
    pub edition: Option<Edition>,
    pub session_seconds: u64,
}

pub static RUNNING: Mutex<RunningStatus> = Mutex::new(RunningStatus {
    running: false,
    edition: None,
    session_seconds: 0,
});
static TRACKER_STARTED: AtomicBool = AtomicBool::new(false);

pub fn running_status() -> RunningStatus {
    RUNNING.lock().map(|r| r.clone()).unwrap_or_default()
}

pub fn is_game_running() -> bool {
    running_status().running
}

pub fn ensure_not_running() -> CmdResult<()> {
    if is_game_running() {
        return Err("Сначала закройте GTA V: пока игра запущена, файлы нельзя менять.".into());
    }
    Ok(())
}

fn install_of(state: &AppState, edition: Edition) -> CmdResult<GameInstall> {
    state
        .snapshot()
        .installs
        .get(edition)
        .cloned()
        .ok_or_else(|| format!("{} не найдена. Нажмите «Найти GTA V».", edition.title()))
}

pub fn game_dir(state: &AppState, edition: Edition) -> CmdResult<PathBuf> {
    let inst = install_of(state, edition)?;
    let p = PathBuf::from(&inst.path);
    if !p.is_dir() {
        return Err(format!("Папка игры не найдена: {}", inst.path));
    }
    Ok(p)
}

fn epic_uri(id: &str, action: &str) -> String {
    format!(
        "com.epicgames.launcher://apps/{}?action={action}&silent=true",
        id.replace(':', "%3A")
    )
}

/// Запуск через официальный клиент. Ничего не обходит: Steam/Epic/Rockstar
/// сами проверяют лицензию и вход в аккаунт.
pub fn launch(app: &AppHandle, state: &AppState, edition: Edition) -> CmdResult<String> {
    if is_game_running() {
        return Err("GTA V уже запущена.".into());
    }
    let inst = install_of(state, edition)?;
    let dir = PathBuf::from(&inst.path);
    let missing = detect::missing_files(&dir, edition);
    if !missing.is_empty() {
        return Err(format!(
            "Не хватает файлов игры: {}. Проверьте целостность файлов в официальном клиенте.",
            missing.join(", ")
        ));
    }

    let how = match inst.source {
        Source::Steam => {
            if detect::steam_root().is_none() {
                return Err(
                    "Steam не найден. Установите Steam или выберите другой источник.".into(),
                );
            }
            let url = format!("steam://rungameid/{}", edition.steam_appid());
            app.opener()
                .open_url(url, None::<&str>)
                .ctx("Не удалось открыть Steam")?;
            "Запуск через Steam…"
        }
        Source::Epic => {
            let id = inst
                .epic_id
                .clone()
                .ok_or("Не найден идентификатор Epic Games. Нажмите «Найти GTA V» ещё раз.")?;
            app.opener()
                .open_url(epic_uri(&id, "launch"), None::<&str>)
                .ctx("Не удалось открыть Epic Games Launcher")?;
            "Запуск через Epic Games Launcher…"
        }
        Source::Rockstar | Source::Unknown => {
            let play = dir.join("PlayGTAV.exe");
            hidden_command(&play)
                .current_dir(&dir)
                .spawn()
                .ctx("Не удалось запустить PlayGTAV.exe")?;
            "Запуск через Rockstar Games Launcher…"
        }
    };

    let now = now_iso();
    state.update(|c| {
        let s = c.stats.get_mut(edition);
        s.launches += 1;
        s.last_launch = Some(now.clone());
        if let Some(p) = c.active_profile_mut() {
            p.launches += 1;
            p.last_launch = Some(now.clone());
        }
    })?;
    Ok(how.to_string())
}

#[derive(Serialize)]
pub struct VerifyResult {
    pub started: bool,
    pub message: String,
}

pub fn verify(app: &AppHandle, state: &AppState, edition: Edition) -> CmdResult<VerifyResult> {
    let inst = install_of(state, edition)?;
    match inst.source {
        Source::Steam => {
            let url = format!("steam://validate/{}", edition.steam_appid());
            app.opener()
                .open_url(url, None::<&str>)
                .ctx("Не удалось открыть Steam")?;
            Ok(VerifyResult {
                started: true,
                message:
                    "Steam начал проверку целостности файлов. Прогресс виден в библиотеке Steam."
                        .into(),
            })
        }
        Source::Epic => {
            let id = inst.epic_id.unwrap_or_default();
            let opened = !id.is_empty()
                && app
                    .opener()
                    .open_url(epic_uri(&id, "verify"), None::<&str>)
                    .is_ok();
            Ok(VerifyResult {
                started: opened,
                message: "Если проверка не началась сама: Epic Games Launcher → Библиотека → «…» у GTA V → Управление → Проверить файлы.".into(),
            })
        }
        Source::Rockstar | Source::Unknown => {
            let started = detect::rockstar_launcher_exe()
                .map(|exe| hidden_command(exe).spawn().is_ok())
                .unwrap_or(false);
            Ok(VerifyResult {
                started: false,
                message: format!(
                    "{}Rockstar Games Launcher: Настройки → Мои установленные игры → Grand Theft Auto V → «Проверить целостность».",
                    if started { "Rockstar Games Launcher открыт. " } else { "" }
                ),
            })
        }
    }
}

fn detect_running(sys: &System) -> Option<Edition> {
    for p in sys.processes().values() {
        let name = p.name().to_string_lossy().to_lowercase();
        if name == "gta5_enhanced.exe" || name == "gta5 enhanced.exe" {
            return Some(Edition::Enhanced);
        }
        if name == "gta5.exe" {
            return Some(Edition::Legacy);
        }
    }
    None
}

/// Фоновый поток: раз в 3 секунды смотрит на процессы и считает время игры.
pub fn start_tracker(app: AppHandle) {
    if TRACKER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
        );
        let mut session: Option<(Edition, Instant)> = None;
        let mut unsaved: u64 = 0;
        let mut last_tick = Instant::now();
        let mut last_save = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(3));
            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing(),
            );
            let current = detect_running(&sys);
            let now = Instant::now();
            let delta = now.duration_since(last_tick).as_secs_f64();
            last_tick = now;

            let state = app.state::<AppState>();
            match (session, current) {
                (None, None) => {}
                (Some((ed, _)), Some(cur)) if ed == cur => {
                    unsaved += delta.round() as u64;
                }
                (prev, cur) => {
                    // Сессия закончилась или сменилась — сохраняем накопленное.
                    if let Some((ed, _)) = prev {
                        flush_time(&state, ed, &mut unsaved);
                    }
                    session = cur.map(|e| (e, now));
                    let _ = app.emit("game-status", status_payload(session));
                }
            }
            if let Some((ed, _)) = session {
                if last_save.elapsed() > Duration::from_secs(60) {
                    flush_time(&state, ed, &mut unsaved);
                    last_save = Instant::now();
                }
            }
            let payload = status_payload(session);
            if let Ok(mut r) = RUNNING.lock() {
                *r = payload.clone();
            }
            if session.is_some() {
                let _ = app.emit("game-status", payload);
            }
        }
    });
}

fn status_payload(session: Option<(Edition, Instant)>) -> RunningStatus {
    match session {
        Some((e, start)) => RunningStatus {
            running: true,
            edition: Some(e),
            session_seconds: start.elapsed().as_secs(),
        },
        None => RunningStatus::default(),
    }
}

fn flush_time(state: &AppState, edition: Edition, unsaved: &mut u64) {
    if *unsaved == 0 {
        return;
    }
    let add = *unsaved;
    *unsaved = 0;
    let _ = state.update(|c| {
        c.stats.get_mut(edition).seconds += add;
        if let Some(p) = c.active_profile_mut() {
            p.seconds += add;
        }
    });
}
