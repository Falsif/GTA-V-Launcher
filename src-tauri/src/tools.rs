//! Новости, проверка обновлений лаунчера, скриншоты, дополнения, фон.

use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::{AppState, Edition};
use crate::launch::game_dir;
use crate::util::{hidden_command, timestamp_slug, CmdResult, Ctx};

fn http() -> CmdResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("GTAV-Launcher/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(12))
        .build()
        .ctx("Ошибка сети")
}

// ---------- Новости (Steam Web API, без ключа) ----------

#[derive(Serialize, Deserialize, Clone)]
pub struct NewsItem {
    pub title: String,
    pub url: String,
    pub summary: String,
    pub date: i64,
    pub source: String,
}

#[derive(Serialize, Deserialize)]
pub struct NewsResult {
    pub items: Vec<NewsItem>,
    pub offline: bool,
    pub fetched_at: i64,
}

fn clean_text(s: &str) -> String {
    let tags =
        Regex::new(r"(?s)<[^>]*>|\[/?[a-zA-Z0-9*]+(=[^\]]*)?\]|\{STEAM_CLAN_IMAGE\}\S*").unwrap();
    let urls = Regex::new(r"https?://\S+").unwrap();
    let t = tags.replace_all(s, " ");
    let t = urls.replace_all(&t, "");
    let t = t
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = t.chars().take(260).collect();
    if t.chars().count() > 260 {
        out.push('…');
    }
    out
}

pub async fn news(state: &AppState, edition: Edition) -> CmdResult<NewsResult> {
    let cache = state.path(&format!("news_{}.json", crate::mods::edition_key(edition)));
    let url = format!(
        "https://api.steampowered.com/ISteamNews/GetNewsForApp/v2/?appid={}&count=8&maxlength=600&format=json",
        edition.steam_appid()
    );
    let fetched: CmdResult<Vec<NewsItem>> = async {
        let v: serde_json::Value = http()?
            .get(&url)
            .send()
            .await
            .ctx("Нет соединения")?
            .error_for_status()
            .ctx("Сервер новостей недоступен")?
            .json()
            .await
            .ctx("Некорректный ответ")?;
        let items = v["appnews"]["newsitems"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|n| NewsItem {
                title: n["title"].as_str().unwrap_or("").to_string(),
                url: n["url"].as_str().unwrap_or("").to_string(),
                summary: clean_text(n["contents"].as_str().unwrap_or("")),
                date: n["date"].as_i64().unwrap_or(0),
                source: n["feedlabel"].as_str().unwrap_or("Steam").to_string(),
            })
            .filter(|n| !n.title.is_empty())
            .collect();
        Ok(items)
    }
    .await;
    match fetched {
        Ok(items) => {
            let r = NewsResult {
                items,
                offline: false,
                fetched_at: chrono::Utc::now().timestamp(),
            };
            if let Ok(s) = serde_json::to_string(&r) {
                let _ = fs::write(&cache, s);
            }
            Ok(r)
        }
        Err(e) => {
            // Без интернета показываем последние сохранённые новости.
            if let Some(mut r) = fs::read_to_string(&cache)
                .ok()
                .and_then(|s| serde_json::from_str::<NewsResult>(&s).ok())
            {
                r.offline = true;
                return Ok(r);
            }
            Err(e)
        }
    }
}

// ---------- Обновления лаунчера (GitHub Releases) ----------

#[derive(Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub url: String,
    pub notes: String,
}

fn parse_ver(s: &str) -> Vec<u64> {
    s.trim_start_matches(['v', 'V'])
        .split(['.', '-'])
        .map(|p| p.parse().unwrap_or(0))
        .take(3)
        .collect()
}

pub async fn check_update(repo: &str) -> CmdResult<UpdateInfo> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    if !Regex::new(r"^[\w.-]+/[\w.-]+$").unwrap().is_match(repo) {
        return Err("Неверный репозиторий обновлений (нужно владелец/репозиторий).".into());
    }
    let resp = http()?
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ctx("Нет соединения с GitHub")?;
    if resp.status().as_u16() == 404 {
        return Ok(UpdateInfo {
            current: current.clone(),
            latest: current,
            available: false,
            url: format!("https://github.com/{repo}/releases"),
            notes: "Релизов пока нет.".into(),
        });
    }
    let v: serde_json::Value = resp
        .error_for_status()
        .ctx("GitHub недоступен")?
        .json()
        .await
        .ctx("Некорректный ответ")?;
    let tag = v["tag_name"].as_str().unwrap_or("0.0.0").to_string();
    let available = parse_ver(&tag) > parse_ver(&current);
    Ok(UpdateInfo {
        current,
        latest: tag.trim_start_matches('v').to_string(),
        available,
        url: v["html_url"].as_str().unwrap_or("").to_string(),
        notes: v["body"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(1500)
            .collect(),
    })
}

// ---------- Скриншоты ----------

pub fn screenshots_dir() -> CmdResult<PathBuf> {
    let base = dirs::picture_dir()
        .or_else(dirs::home_dir)
        .ok_or("Не найдена папка «Изображения»")?;
    let dir = base.join("GTA V Launcher");
    fs::create_dir_all(&dir).ctx("Не удалось создать папку для скриншотов")?;
    Ok(dir)
}

/// Снимок основного монитора через .NET (System.Drawing). В эксклюзивном
/// полноэкранном режиме DirectX снимок может получиться чёрным — тогда
/// используйте «Без рамки» или встроенную функцию клиента (Steam: F12).
pub fn screenshot() -> CmdResult<String> {
    if !cfg!(windows) {
        return Err("Скриншоты доступны только в Windows.".into());
    }
    let file = screenshots_dir()?.join(format!("GTAV_{}.png", timestamp_slug()));
    let path = file.to_string_lossy().replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing;\
         Add-Type -Name D -Namespace W -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool SetProcessDPIAware();';\
         [void][W.D]::SetProcessDPIAware();\
         $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds;\
         $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height;\
         $g=[System.Drawing.Graphics]::FromImage($bmp);\
         $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size);\
         $bmp.Save('{path}',[System.Drawing.Imaging.ImageFormat]::Png);\
         $g.Dispose();$bmp.Dispose()"
    );
    // -EncodedCommand (UTF-16LE + base64): без проблем с кавычками и кириллицей в пути.
    let utf16: Vec<u8> = script
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let out = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded,
        ])
        .output()
        .ctx("Не удалось запустить PowerShell")?;
    if !out.status.success() || !file.is_file() {
        return Err(format!(
            "Скриншот не сделан: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(file.to_string_lossy().into())
}

// ---------- Дополнения (DLC-пакеты) ----------

#[derive(Serialize)]
pub struct Addons {
    pub official: Vec<String>,
    pub modded: Vec<String>,
    pub mods_folder: bool,
}

fn list_dirs(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(p)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    e.path().is_dir()
                        || e.file_name()
                            .to_string_lossy()
                            .to_lowercase()
                            .ends_with(".rpf")
                })
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|s| s.to_lowercase());
    v
}

pub fn addons(state: &AppState, edition: Edition) -> CmdResult<Addons> {
    let g = game_dir(state, edition)?;
    let official = list_dirs(&g.join("update").join("x64").join("dlcpacks"));
    let modded_dir = g.join("mods").join("update").join("x64").join("dlcpacks");
    let modded: Vec<String> = list_dirs(&modded_dir)
        .into_iter()
        .filter(|n| !official.iter().any(|o| o.eq_ignore_ascii_case(n)))
        .collect();
    Ok(Addons {
        official,
        modded,
        mods_folder: g.join("mods").is_dir(),
    })
}

// ---------- Фон ----------

pub fn data_url(path: &Path) -> CmdResult<String> {
    let bytes = fs::read(path).ctx("Не удалось прочитать изображение")?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn check_image(path: &Path, max_mb: u64) -> CmdResult<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
        return Err("Поддерживаются изображения PNG, JPG, WEBP и GIF.".into());
    }
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size > max_mb * 1024 * 1024 {
        return Err(format!("Изображение больше {max_mb} МБ."));
    }
    Ok(())
}

pub fn background_file(state: &AppState) -> Option<PathBuf> {
    fs::read_dir(&state.data_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_stem().map(|s| s == "background").unwrap_or(false))
}

pub fn set_background(state: &AppState, src: Option<&Path>) -> CmdResult<Option<String>> {
    if let Some(old) = background_file(state) {
        let _ = fs::remove_file(old);
    }
    let Some(src) = src else {
        state.update(|c| c.launcher.has_background = false)?;
        return Ok(None);
    };
    check_image(src, 15)?;
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let dst = state.data_dir.join(format!("background.{ext}"));
    fs::copy(src, &dst).ctx("Не удалось сохранить фон")?;
    state.update(|c| c.launcher.has_background = true)?;
    data_url(&dst).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_cleanup() {
        assert_eq!(
            clean_text("[b]Hello[/b] <p>world</p> &amp; more"),
            "Hello world & more"
        );
    }
    #[test]
    fn versions() {
        assert!(parse_ver("v1.2.0") > parse_ver("1.1.9"));
        assert!(parse_ver("1.0.0") == parse_ver("1.0.0"));
    }
}
