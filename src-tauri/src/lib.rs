//! GTA V Launcher — неофициальный лаунчер. Запускает игру только через официальные
//! клиенты (Steam / Epic Games / Rockstar Games Launcher) и ничего не обходит.

mod config;
mod detect;
mod game_settings;
mod install;
mod launch;
mod mods;
mod tools;
mod util;

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_opener::OpenerExt;

use config::{AppState, Config, Edition, LauncherSettings, Profile, Source};
use util::{now_iso, CmdResult, Ctx};

// ======================= Состояние и сведения об игре =======================

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.snapshot()
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

#[derive(Serialize)]
struct GameInfo {
    edition: Edition,
    installed: bool,
    path: Option<String>,
    source: Option<Source>,
    auto: bool,
    version: Option<String>,
    missing: Vec<String>,
    documents: String,
    settings_exists: bool,
}

#[tauri::command(async)]
fn get_game_info(state: State<AppState>, edition: Edition) -> GameInfo {
    let cfg = state.snapshot();
    let inst = cfg.installs.get(edition).cloned();
    let documents = game_settings::documents_dir(edition)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let settings_exists = game_settings::settings_path(edition)
        .map(|p| p.is_file())
        .unwrap_or(false);
    match inst {
        None => GameInfo {
            edition,
            installed: false,
            path: None,
            source: None,
            auto: false,
            version: None,
            missing: vec![],
            documents,
            settings_exists,
        },
        Some(i) => {
            let dir = PathBuf::from(&i.path);
            let missing = detect::missing_files(&dir, edition);
            let version = detect::exe_path(&dir, edition).and_then(|e| detect::exe_version(&e));
            GameInfo {
                edition,
                installed: missing.is_empty(),
                path: Some(i.path),
                source: Some(i.source),
                auto: i.auto,
                version,
                missing,
                documents,
                settings_exists,
            }
        }
    }
}

#[tauri::command]
fn get_running_status() -> launch::RunningStatus {
    launch::running_status()
}

// ======================= Поиск игры =======================

#[derive(Serialize)]
struct DetectSummary {
    found: Vec<detect::Found>,
    applied: Vec<Edition>,
    config: Config,
}

#[tauri::command(async)]
fn auto_detect(state: State<AppState>, overwrite: bool) -> CmdResult<DetectSummary> {
    let found = detect::detect_all();
    let mut applied = vec![];
    state.update(|c| {
        for f in &found {
            if applied.contains(&f.edition) {
                continue;
            }
            let current_ok = c
                .installs
                .get(f.edition)
                .map(|i| detect::edition_of(Path::new(&i.path)) == Some(f.edition))
                .unwrap_or(false);
            if overwrite || !current_ok {
                c.installs.set(f.edition, Some(f.install.clone()));
                applied.push(f.edition);
            }
        }
        // Если выбранной редакции нет, а другая есть — переключаемся.
        if c.installs.get(c.selected_edition).is_none() {
            if let Some(e) = Edition::ALL
                .into_iter()
                .find(|e| c.installs.get(*e).is_some())
            {
                c.selected_edition = e;
            }
        }
        c.first_run_done = true;
    })?;
    Ok(DetectSummary {
        found,
        applied,
        config: state.snapshot(),
    })
}

#[tauri::command(async)]
fn select_game_folder(state: State<AppState>, path: String) -> CmdResult<Config> {
    let (edition, inst) = detect::classify_folder(Path::new(&path))?;
    state.update(|c| {
        c.installs.set(edition, Some(inst));
        c.selected_edition = edition;
        c.first_run_done = true;
    })?;
    Ok(state.snapshot())
}

#[tauri::command]
fn clear_game_path(state: State<AppState>, edition: Edition) -> CmdResult<Config> {
    state.update(|c| c.installs.set(edition, None))?;
    Ok(state.snapshot())
}

#[tauri::command]
fn set_game_source(state: State<AppState>, edition: Edition, source: Source) -> CmdResult<Config> {
    state.update(|c| {
        let mut inst = c.installs.get(edition).cloned();
        if let Some(i) = inst.as_mut() {
            i.source = source;
            if source == Source::Epic && i.epic_id.is_none() {
                i.epic_id = detect::epic_items()
                    .into_iter()
                    .find(|e| util::same_path(&e.location, &i.path))
                    .map(|e| e.launch_id);
            }
        }
        c.installs.set(edition, inst);
    })?;
    Ok(state.snapshot())
}

#[tauri::command]
fn set_selected_edition(state: State<AppState>, edition: Edition) -> CmdResult<Config> {
    state.update(|c| {
        c.selected_edition = edition;
        if let Some(p) = c.active_profile_mut() {
            p.edition = edition;
        }
    })?;
    Ok(state.snapshot())
}

// ======================= Запуск =======================

#[tauri::command(async)]
fn launch_game(app: AppHandle, state: State<AppState>, edition: Edition) -> CmdResult<String> {
    launch::launch(&app, &state, edition)
}

#[tauri::command(async)]
fn verify_game(
    app: AppHandle,
    state: State<AppState>,
    edition: Edition,
) -> CmdResult<launch::VerifyResult> {
    launch::verify(&app, &state, edition)
}

#[tauri::command(async)]
fn mods_preflight(state: State<AppState>, edition: Edition) -> mods::Preflight {
    mods::preflight(&state, edition)
}

#[tauri::command(async)]
fn system_check() -> install::SystemCheck {
    install::system_check()
}

#[tauri::command(async)]
fn install_game(
    app: AppHandle,
    edition: Edition,
    store: String,
) -> CmdResult<install::InstallStart> {
    install::start_install(&app, edition, &store)
}

// ======================= Моды =======================

#[tauri::command(async)]
fn inspect_mod(path: String, edition: Edition) -> CmdResult<mods::ModInspect> {
    mods::inspect(Path::new(&path), edition)
}

#[tauri::command(async)]
fn install_mod(state: State<AppState>, req: mods::InstallRequest) -> CmdResult<mods::EnableResult> {
    mods::install(&state, req)
}

#[tauri::command(async)]
fn enable_mod(state: State<AppState>, id: String, force: bool) -> CmdResult<mods::EnableResult> {
    mods::enable(&state, &id, force)
}

#[tauri::command(async)]
fn disable_mod(state: State<AppState>, id: String) -> CmdResult<()> {
    mods::disable(&state, &id)
}

#[tauri::command(async)]
fn remove_mod(state: State<AppState>, id: String) -> CmdResult<()> {
    mods::remove(&state, &id)
}

#[tauri::command(async)]
fn disable_all_mods(state: State<AppState>, edition: Edition) -> CmdResult<usize> {
    mods::disable_all(&state, edition)
}

#[tauri::command]
fn update_mod(
    state: State<AppState>,
    id: String,
    name: String,
    version: String,
    description: String,
    category: String,
) -> CmdResult<()> {
    mods::update_meta(&state, &id, name, version, description, category)
}

#[tauri::command(async)]
fn scan_external_mods(
    state: State<AppState>,
    edition: Edition,
) -> CmdResult<Vec<mods::ExternalItem>> {
    mods::scan_external(&state, edition)
}

#[tauri::command(async)]
fn check_mods(state: State<AppState>, edition: Edition) -> CmdResult<Vec<mods::ModWarning>> {
    mods::check(&state, edition)
}

// ======================= Настройки игры =======================

#[tauri::command(async)]
fn read_game_settings(edition: Edition) -> CmdResult<game_settings::GameSettings> {
    game_settings::read(edition)
}

#[tauri::command(async)]
fn write_game_settings(
    state: State<AppState>,
    edition: Edition,
    changes: BTreeMap<String, String>,
) -> CmdResult<usize> {
    game_settings::write(&state, edition, changes)
}

#[tauri::command(async)]
fn read_commandline(state: State<AppState>, edition: Edition) -> CmdResult<String> {
    game_settings::read_commandline(&state, edition)
}

#[tauri::command(async)]
fn write_commandline(state: State<AppState>, edition: Edition, text: String) -> CmdResult<()> {
    game_settings::write_commandline(&state, edition, text)
}

#[tauri::command]
fn list_settings_profiles(state: State<AppState>, edition: Edition) -> Vec<String> {
    game_settings::list_profiles(&state, edition)
}

#[tauri::command]
fn save_settings_profile(state: State<AppState>, edition: Edition, name: String) -> CmdResult<()> {
    game_settings::save_profile(&state, edition, &name)
}

#[tauri::command]
fn apply_settings_profile(state: State<AppState>, edition: Edition, name: String) -> CmdResult<()> {
    game_settings::apply_profile(&state, edition, &name)
}

#[tauri::command]
fn delete_settings_profile(
    state: State<AppState>,
    edition: Edition,
    name: String,
) -> CmdResult<()> {
    game_settings::delete_profile(&state, edition, &name)
}

#[tauri::command(async)]
fn create_config_backup(state: State<AppState>, edition: Edition) -> CmdResult<String> {
    game_settings::backup_now(&state, edition, "manual")
}

#[tauri::command]
fn list_config_backups(state: State<AppState>, edition: Edition) -> Vec<game_settings::BackupInfo> {
    game_settings::list_backups(&state, edition)
}

#[tauri::command(async)]
fn restore_config_backup(state: State<AppState>, edition: Edition, id: String) -> CmdResult<()> {
    game_settings::restore_backup(&state, edition, &id)
}

#[tauri::command]
fn delete_config_backup(state: State<AppState>, edition: Edition, id: String) -> CmdResult<()> {
    game_settings::delete_backup(&state, edition, &id)
}

// ======================= Профили лаунчера =======================

#[tauri::command]
fn create_profile(
    state: State<AppState>,
    name: String,
    avatar: String,
    edition: Edition,
) -> CmdResult<Config> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 32 {
        return Err("Имя профиля: от 1 до 32 символов.".into());
    }
    state.update(|c| {
        let p = Profile {
            id: uuid::Uuid::new_v4().simple().to_string()[..10].to_string(),
            name,
            avatar,
            edition,
            theme: c.launcher.theme.clone(),
            mod_ids: None,
            created_at: now_iso(),
            ..Default::default()
        };
        if c.active_profile.is_none() {
            c.active_profile = Some(p.id.clone());
        }
        c.profiles.push(p);
    })?;
    Ok(state.snapshot())
}

#[tauri::command]
fn update_profile(
    state: State<AppState>,
    id: String,
    name: String,
    avatar: String,
    edition: Edition,
    theme: String,
) -> CmdResult<Config> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 32 {
        return Err("Имя профиля: от 1 до 32 символов.".into());
    }
    state.update(|c| {
        let active = c.active_profile.as_deref() == Some(id.as_str());
        if let Some(p) = c.profiles.iter_mut().find(|p| p.id == id) {
            p.name = name;
            p.avatar = avatar;
            p.edition = edition;
            p.theme = theme.clone();
        }
        if active {
            c.launcher.theme = theme;
            c.selected_edition = edition;
        }
    })?;
    Ok(state.snapshot())
}

#[tauri::command]
fn delete_profile(state: State<AppState>, id: String) -> CmdResult<Config> {
    state.update(|c| {
        c.profiles.retain(|p| p.id != id);
        if c.active_profile.as_deref() == Some(id.as_str()) {
            c.active_profile = c.profiles.first().map(|p| p.id.clone());
        }
    })?;
    Ok(state.snapshot())
}

#[derive(Serialize)]
struct SwitchResult {
    config: Config,
    notes: Vec<String>,
}

#[tauri::command(async)]
fn switch_profile(state: State<AppState>, id: String) -> CmdResult<SwitchResult> {
    let p = state
        .snapshot()
        .profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("Профиль не найден")?;
    let mut notes = vec![];
    if let Some(ids) = &p.mod_ids {
        if state.snapshot().installs.get(p.edition).is_some() {
            notes = mods::apply_set(&state, p.edition, ids)?;
        }
    }
    state.update(|c| {
        c.active_profile = Some(p.id.clone());
        c.selected_edition = p.edition;
        if !p.theme.is_empty() {
            c.launcher.theme = p.theme.clone();
        }
    })?;
    Ok(SwitchResult {
        config: state.snapshot(),
        notes,
    })
}

/// Запомнить в профиле текущий набор включённых модов (или перестать управлять модами).
#[tauri::command]
fn set_profile_mods(state: State<AppState>, id: String, capture: bool) -> CmdResult<Config> {
    state.update(|c| {
        let enabled: Vec<(Edition, String)> = c
            .mods
            .iter()
            .filter(|m| m.enabled)
            .map(|m| (m.edition, m.id.clone()))
            .collect();
        if let Some(p) = c.profiles.iter_mut().find(|p| p.id == id) {
            p.mod_ids = if capture {
                Some(
                    enabled
                        .into_iter()
                        .filter(|(e, _)| *e == p.edition)
                        .map(|(_, id)| id)
                        .collect(),
                )
            } else {
                None
            };
        }
    })?;
    Ok(state.snapshot())
}

#[tauri::command(async)]
fn load_avatar(path: String) -> CmdResult<String> {
    let p = Path::new(&path);
    tools::check_image(p, 2)?;
    tools::data_url(p)
}

// ======================= Настройки лаунчера и инструменты =======================

#[tauri::command]
fn update_launcher_settings(
    state: State<AppState>,
    settings: LauncherSettings,
) -> CmdResult<Config> {
    state.update(|c| {
        let has_bg = c.launcher.has_background;
        c.launcher = settings;
        c.launcher.has_background = has_bg;
        let theme = c.launcher.theme.clone();
        if let Some(p) = c.active_profile_mut() {
            p.theme = theme;
        }
    })?;
    Ok(state.snapshot())
}

#[tauri::command(async)]
fn get_background(state: State<AppState>) -> Option<String> {
    tools::background_file(&state).and_then(|p| tools::data_url(&p).ok())
}

#[tauri::command(async)]
fn set_background(state: State<AppState>, path: Option<String>) -> CmdResult<Option<String>> {
    tools::set_background(&state, path.as_deref().map(Path::new))
}

#[tauri::command]
async fn fetch_news(state: State<'_, AppState>, edition: Edition) -> CmdResult<tools::NewsResult> {
    tools::news(&state, edition).await
}

#[tauri::command]
async fn check_launcher_update(state: State<'_, AppState>) -> CmdResult<tools::UpdateInfo> {
    let repo = state.snapshot().launcher.update_repo;
    tools::check_update(&repo).await
}

#[tauri::command]
async fn take_screenshot(app: AppHandle, hide_window: bool) -> CmdResult<String> {
    if !launch::is_game_running() {
        return Err("Скриншот доступен, когда игра запущена.".into());
    }
    let win = app.get_webview_window("main");
    if hide_window {
        if let Some(w) = &win {
            let _ = w.hide();
        }
        tokio_sleep(900).await;
    }
    let r = tauri::async_runtime::spawn_blocking(tools::screenshot)
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
    if hide_window {
        if let Some(w) = &win {
            let _ = w.show();
        }
    }
    r
}

async fn tokio_sleep(ms: u64) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(ms))
    })
    .await;
}

#[tauri::command(async)]
fn list_addons(state: State<AppState>, edition: Edition) -> CmdResult<tools::Addons> {
    tools::addons(&state, edition)
}

#[tauri::command(async)]
fn open_location(
    app: AppHandle,
    state: State<AppState>,
    kind: String,
    edition: Edition,
) -> CmdResult<()> {
    let path: PathBuf = match kind.as_str() {
        "game" => launch::game_dir(&state, edition)?,
        "documents" => {
            let p = game_settings::documents_dir(edition)?;
            if !p.is_dir() {
                return Err("Папка в «Документах» появится после первого запуска игры.".into());
            }
            p
        }
        "scripts" => {
            let p = launch::game_dir(&state, edition)?.join("scripts");
            if !p.is_dir() {
                return Err("Папки scripts нет — моды-скрипты не установлены.".into());
            }
            p
        }
        "screenshots" => tools::screenshots_dir()?,
        "data" => state.data_dir.clone(),
        "backups" => state.path("config_backups"),
        "mods_storage" => state.path("mods"),
        _ => return Err("Неизвестная папка".into()),
    };
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .ctx("Не удалось открыть папку")
}

#[tauri::command]
fn open_file(app: AppHandle, path: String) -> CmdResult<()> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("Файл не найден".into());
    }
    app.opener()
        .open_path(path, None::<&str>)
        .ctx("Не удалось открыть")
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> CmdResult<()> {
    if !url.starts_with("https://") {
        return Err("Разрешены только https-ссылки".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .ctx("Не удалось открыть ссылку")
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ======================= Трей, горячая клавиша, запуск =======================

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Открыть лаунчер", true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play", "Играть", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &play, &sep, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("GTA V Launcher")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "play" => {
                show_main(app);
                let _ = app.emit("tray-play", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn setup_hotkey(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };
    let shot = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS);
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, shortcut, event| {
            if shortcut != &shot || event.state() != ShortcutState::Pressed {
                return;
            }
            let enabled = app
                .state::<AppState>()
                .snapshot()
                .launcher
                .screenshot_hotkey;
            if !enabled || !launch::is_game_running() {
                return;
            }
            let app = app.clone();
            std::thread::spawn(move || {
                let _ = match tools::screenshot() {
                    Ok(path) => app.emit(
                        "screenshot",
                        serde_json::json!({ "ok": true, "path": path }),
                    ),
                    Err(e) => {
                        app.emit("screenshot", serde_json::json!({ "ok": false, "error": e }))
                    }
                };
            });
        })
        .build();
    if app.plugin(plugin).is_ok() {
        // Сочетание может быть занято другой программой — тогда просто работаем без него.
        let _ = app.global_shortcut().register(shot);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("gtav-launcher"));
            app.manage(AppState::load(data_dir));
            let handle = app.handle().clone();
            setup_tray(&handle)?;
            setup_hotkey(&handle);
            launch::start_tracker(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let to_tray = window
                    .app_handle()
                    .state::<AppState>()
                    .snapshot()
                    .launcher
                    .close_to_tray;
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            app_version,
            get_game_info,
            get_running_status,
            auto_detect,
            select_game_folder,
            clear_game_path,
            set_game_source,
            set_selected_edition,
            launch_game,
            verify_game,
            mods_preflight,
            system_check,
            install_game,
            inspect_mod,
            install_mod,
            enable_mod,
            disable_mod,
            remove_mod,
            disable_all_mods,
            update_mod,
            scan_external_mods,
            check_mods,
            read_game_settings,
            write_game_settings,
            read_commandline,
            write_commandline,
            list_settings_profiles,
            save_settings_profile,
            apply_settings_profile,
            delete_settings_profile,
            create_config_backup,
            list_config_backups,
            restore_config_backup,
            delete_config_backup,
            create_profile,
            update_profile,
            delete_profile,
            switch_profile,
            set_profile_mods,
            load_avatar,
            update_launcher_settings,
            get_background,
            set_background,
            fetch_news,
            check_launcher_update,
            take_screenshot,
            list_addons,
            open_location,
            open_file,
            open_url,
            quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка запуска GTA V Launcher");
}
