#![allow(dead_code)]

pub mod settings;

use anyhow::{Context, Result};
use settings::Settings;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Cannot determine HOME")?;

    Ok(home.join("Library/Application Support/Yeti3-Cleaner"))
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

pub fn load() -> Result<Settings> {
    let path = settings_path()?;

    if !path.exists() {
        let settings = Settings::default();
        save(&settings)?;
        return Ok(settings);
    }

    let data = fs::read_to_string(&path).with_context(|| format!("Read {}", path.display()))?;

    let settings =
        serde_json::from_str(&data).with_context(|| format!("Parse {}", path.display()))?;

    Ok(settings)
}

pub fn save(settings: &Settings) -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)?;

    let path = settings_path()?;
    let tmp = path.with_extension("json.tmp");

    let data = serde_json::to_vec_pretty(settings)?;

    fs::write(&tmp, data)?;
    fs::rename(tmp, path)?;

    Ok(())
}

/*
 * HARD DENYLIST.
 *
 * Settings and GUI must NEVER be able to override these.
 */
pub fn is_protected(path: &Path) -> bool {
    let Some(home) = dirs::home_dir() else {
        return true;
    };

    let protected = [
        home.join(".ssh"),
        home.join(".gnupg"),
        home.join(".aws"),
        home.join(".docker"),
        home.join(".kube"),
        home.join(".codex"),
        home.join("Library/Application Support/Yeti3-Cleaner"),
        home.join("Downloads"),
        home.join("Documents"),
        home.join("GIT"),
        home.join("Pictures"),
        home.join("Movies"),
        home.join("Music"),
        home.join("Desktop"),
        home.join("Library/Mobile Documents"),
        home.join("Library/CloudStorage"),
        home.join("Library/Mail"),
        home.join("Library/Messages"),
        home.join("Library/Keychains"),
        home.join("Library/Group Containers/6N38VWS5BX.ru.keepcoder.Telegram"),
        home.join("Library/Group Containers/group.net.whatsapp.WhatsApp.shared"),
    ];

    path.components().any(|part| part.as_os_str() == ".git") || path == home || path == Path::new("/") || !path.starts_with(&home)
        || protected.iter().any(|p| path == p || path.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloads_are_always_protected() {
        let home = dirs::home_dir().unwrap();

        assert!(is_protected(&home.join("Downloads/test.dmg")));
    }

    #[test]
    fn git_is_always_protected() {
        let home = dirs::home_dir().unwrap();

        assert!(is_protected(&home.join("GIT/project/.git/objects")));
    }

    #[test]
    fn documents_are_always_protected() {
        let home = dirs::home_dir().unwrap();

        assert!(is_protected(&home.join("Documents/report.pdf")));
    }
}
