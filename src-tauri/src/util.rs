//! Общие вспомогательные функции.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub type CmdResult<T> = Result<T, String>;

/// Добавляет понятный русский контекст к ошибке.
pub trait Ctx<T> {
    fn ctx(self, msg: &str) -> CmdResult<T>;
}

impl<T, E: std::fmt::Display> Ctx<T> for Result<T, E> {
    fn ctx(self, msg: &str) -> CmdResult<T> {
        self.map_err(|e| format!("{msg}: {e}"))
    }
}

pub fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn timestamp_slug() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

/// Команда без всплывающего окна консоли (на Windows).
pub fn hidden_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Безопасный относительный путь: без `..`, без корня и букв дисков. Разделители — `/`.
pub fn sanitize_rel(p: &str) -> Option<String> {
    let p = p.replace('\\', "/");
    let mut parts: Vec<String> = Vec::new();
    for comp in Path::new(&p).components() {
        match comp {
            Component::Normal(s) => {
                let s = s.to_string_lossy().to_string();
                if s.contains(':') || s.trim().is_empty() {
                    return None;
                }
                parts.push(s);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

pub fn join_rel(base: &Path, rel: &str) -> PathBuf {
    let mut p = base.to_path_buf();
    for part in rel.split('/') {
        p.push(part);
    }
    p
}

pub fn copy_file_mkdirs(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst).map(|_| ())
}

/// Сравнение путей без учёта регистра (Windows).
pub fn same_path(a: &str, b: &str) -> bool {
    let n = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    n(a) == n(b)
}

pub fn file_exists_ci(dir: &Path, name: &str) -> bool {
    if dir.join(name).is_file() {
        return true;
    }
    // На Windows регистр не важен, но на всякий случай проверяем вручную.
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().eq_ignore_ascii_case(name) && e.path().is_file() {
                return true;
            }
        }
    }
    false
}

/// Значение параметра реестра (строка). `key` вида `HKLM\\SOFTWARE\\...` или `HKCU\\...`.
#[cfg(windows)]
pub fn reg_query_value(key: &str, value: &str) -> Option<String> {
    let (root, sub) = split_reg_key(key)?;
    let k = root.open_subkey(sub).ok()?;
    let v: String = k.get_value(value).ok()?;
    let v = v.trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[cfg(not(windows))]
pub fn reg_query_value(_key: &str, _value: &str) -> Option<String> {
    None
}

/// Полные пути подразделов ключа реестра.
#[cfg(windows)]
pub fn reg_subkeys(key: &str) -> Vec<String> {
    let Some((root, sub)) = split_reg_key(key) else {
        return vec![];
    };
    let Ok(k) = root.open_subkey(sub) else {
        return vec![];
    };
    k.enum_keys()
        .flatten()
        .map(|name| format!("{}\\{}", key.trim_end_matches('\\'), name))
        .collect()
}

#[cfg(not(windows))]
pub fn reg_subkeys(_key: &str) -> Vec<String> {
    vec![]
}

#[cfg(windows)]
fn split_reg_key(key: &str) -> Option<(winreg::RegKey, &str)> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let (root, sub) = key.split_once('\\')?;
    let hive = match root.to_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
        _ => return None,
    };
    Some((winreg::RegKey::predef(hive), sub))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize() {
        assert_eq!(
            sanitize_rel("scripts\\a.dll").as_deref(),
            Some("scripts/a.dll")
        );
        assert_eq!(sanitize_rel("./x/y.asi").as_deref(), Some("x/y.asi"));
        assert!(sanitize_rel("../evil.dll").is_none());
        assert!(sanitize_rel("/abs/evil.dll").is_none());
        assert!(sanitize_rel("C:/evil.dll").is_none());
        assert!(sanitize_rel("").is_none());
    }
}
