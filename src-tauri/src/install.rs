//! Установка GTA V через официальный клиент и проверка компьютера перед установкой.
//! Лаунчер сам файлы игры не скачивает — это делает Steam / Epic / Rockstar.

use serde::Serialize;
use std::path::PathBuf;
use sysinfo::{Disks, System};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::config::Edition;
use crate::detect;
use crate::util::{hidden_command, CmdResult, Ctx};

/// Сколько места нужно для установки (с запасом под обновления), ГБ.
pub const NEED_GB: f64 = 120.0;

#[derive(Serialize)]
pub struct DriveInfo {
    pub mount: String,
    pub free_gb: f64,
    pub total_gb: f64,
    pub enough: bool,
    pub removable: bool,
}

#[derive(Serialize)]
pub struct SystemCheck {
    pub cpu: String,
    pub cores: usize,
    pub ram_gb: f64,
    pub gpus: Vec<String>,
    pub drives: Vec<DriveInfo>,
    pub os: String,
    pub steam: bool,
    pub epic: bool,
    pub rockstar: bool,
}

pub fn system_check() -> SystemCheck {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let cpu = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Не удалось определить".into());
    let gb = |b: u64| (b as f64 / 1024f64.powi(3) * 10.0).round() / 10.0;

    let disks = Disks::new_with_refreshed_list();
    let mut drives: Vec<DriveInfo> = vec![];
    for d in disks.list() {
        let mount = d.mount_point().to_string_lossy().to_string();
        if drives.iter().any(|x| x.mount == mount) || d.total_space() == 0 {
            continue;
        }
        let free = gb(d.available_space());
        drives.push(DriveInfo {
            mount,
            free_gb: free,
            total_gb: gb(d.total_space()),
            enough: free >= NEED_GB,
            removable: d.is_removable(),
        });
    }
    drives.sort_by(|a, b| a.mount.cmp(&b.mount));

    SystemCheck {
        cpu,
        cores: sys.cpus().len(),
        ram_gb: gb(sys.total_memory()),
        gpus: gpu_names(),
        drives,
        os: System::long_os_version().unwrap_or_else(|| "Windows".into()),
        steam: detect::steam_root().is_some(),
        epic: epic_exe().is_some(),
        rockstar: detect::rockstar_launcher_exe().is_some(),
    }
}

/// Названия видеокарт через PowerShell/CIM (только Windows).
fn gpu_names() -> Vec<String> {
    if !cfg!(windows) {
        return vec![];
    }
    use base64::Engine;
    let script = "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
                  Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name }";
    let utf16: Vec<u8> = script
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    hidden_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn epic_exe() -> Option<PathBuf> {
    let pf86 =
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into());
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    [pf86, pf]
        .iter()
        .map(|b| {
            PathBuf::from(b)
                .join("Epic Games")
                .join("Launcher")
                .join("Portal")
                .join("Binaries")
                .join("Win64")
                .join("EpicGamesLauncher.exe")
        })
        .find(|p| p.is_file())
}

#[derive(Serialize)]
pub struct InstallStart {
    pub opened: bool,
    pub message: String,
}

/// Открывает установку в официальном клиенте.
pub fn start_install(app: &AppHandle, edition: Edition, store: &str) -> CmdResult<InstallStart> {
    match store {
        "steam" => {
            if detect::steam_root().is_none() {
                app.opener()
                    .open_url("https://store.steampowered.com/about/", None::<&str>)
                    .ctx("Не удалось открыть сайт Steam")?;
                return Ok(InstallStart {
                    opened: false,
                    message: "Steam не установлен. Открыт сайт Steam — скачайте и установите его, войдите в аккаунт, где куплена GTA V, затем нажмите «Установить» ещё раз.".into(),
                });
            }
            let url = format!("steam://install/{}", edition.steam_appid());
            app.opener()
                .open_url(url, None::<&str>)
                .ctx("Не удалось открыть Steam")?;
            Ok(InstallStart {
                opened: true,
                message: "В Steam откроется окно установки. Выберите диск, где достаточно места, и нажмите «Установить». Если окно не появилось — игра не куплена на этом аккаунте Steam.".into(),
            })
        }
        "epic" => {
            let exe = epic_exe();
            match exe {
                Some(p) => {
                    hidden_command(p)
                        .spawn()
                        .ctx("Не удалось запустить Epic Games Launcher")?;
                    Ok(InstallStart {
                        opened: true,
                        message: "Epic Games Launcher открыт: Библиотека → Grand Theft Auto V → Установить. Выберите папку на диске, где достаточно места.".into(),
                    })
                }
                None => {
                    app.opener()
                        .open_url("https://store.epicgames.com/download", None::<&str>)
                        .ctx("Не удалось открыть сайт Epic Games")?;
                    Ok(InstallStart {
                        opened: false,
                        message: "Epic Games Launcher не установлен. Открыт сайт для его загрузки."
                            .into(),
                    })
                }
            }
        }
        "rockstar" => match detect::rockstar_launcher_exe() {
            Some(p) => {
                hidden_command(p)
                    .spawn()
                    .ctx("Не удалось запустить Rockstar Games Launcher")?;
                Ok(InstallStart {
                    opened: true,
                    message: "Rockstar Games Launcher открыт: найдите Grand Theft Auto V в библиотеке и нажмите «Установить».".into(),
                })
            }
            None => {
                app.opener()
                    .open_url("https://www.rockstargames.com/downloads", None::<&str>)
                    .ctx("Не удалось открыть сайт Rockstar")?;
                Ok(InstallStart {
                    opened: false,
                    message: "Rockstar Games Launcher не установлен. Открыт сайт для его загрузки."
                        .into(),
                })
            }
        },
        _ => Err("Неизвестный магазин".into()),
    }
}
