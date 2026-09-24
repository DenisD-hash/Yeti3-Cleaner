use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
use crate::config::{self, settings::Settings};

#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FolderRules { pub include: Vec<PathBuf>, pub exclude: Vec<PathBuf> }
impl FolderRules {
    pub fn excludes(&self, path: &Path) -> bool {
        self.exclude.iter().any(|p| path.starts_with(p))
    }
    // A recursive deletion must never swallow an excluded descendant.
    pub fn overlaps_exclusion(&self, path: &Path) -> bool {
        self.exclude.iter().any(|p| path.starts_with(p) || p.starts_with(path))
    }
}
pub fn load() -> Result<FolderRules> {
    let path = config::config_dir()?.join("folders.json");
    if !path.exists() { return Ok(FolderRules::default()); }
    let rules: FolderRules = serde_json::from_slice(&fs::read(path)?)?;
    for p in rules.include.iter().chain(&rules.exclude) {
        if !p.is_absolute() || p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            bail!("Ожидается абсолютный путь без '..': {}", p.display());
        }
    }
    Ok(rules)
}
pub fn validate_custom(path: &Path) -> Result<()> {
    let h = dirs::home_dir().context("HOME unavailable")?;
    let canonical = fs::canonicalize(path)?;
    if path != canonical || !canonical.is_dir() || config::is_protected(&canonical) {
        bail!("Каталог защищён, не существует или содержит символическую ссылку");
    }
    // Broad ancestors could contain credentials, sessions or ordinary documents.
    if ["Library", "Library/Application Support", "Library/Containers", "Library/Group Containers", ".ssh", ".gnupg", ".aws", ".config", ".codex", ".docker", ".kube"].iter().any(|p| {
        let protected = h.join(p);
        canonical == protected || protected.starts_with(&canonical)
            || ([".ssh", ".gnupg", ".aws", ".config", ".codex", ".docker", ".kube"].contains(p) && canonical.starts_with(protected))
    }) { bail!("Этот каталог содержит настройки или защищённые данные; выберите конкретную папку кэша"); }
    Ok(())
}
pub fn preset_enabled(label: &str, s: &Settings) -> bool {
    match label {
        "Trash" => s.macos.trash,
        "Application caches" | "AppSupport caches" => s.macos.application_caches,
        "Logs" => s.macos.logs,
        "Crash reports" => s.macos.crash_reports,
        "CoreML cache" => s.ai_ml.coreml,
        "Xcode DerivedData" => s.development.xcode_derived_data,
        "Xcode SourcePackages" => s.development.xcode_source_packages,
        "Cargo cache" | "Cargo sources" | "Cargo git cache" => s.development.cargo,
        "npm cache" => s.development.npm,
        "npx cache" => s.development.npx,
        "pip cache" => s.development.pip,
        "uv cache" => s.development.uv,
        "Yarn cache" => s.development.yarn,
        "pnpm cache" => s.development.pnpm,
        "CocoaPods cache" => s.development.cocoapods,
        "Gradle cache" => s.development.gradle,
        _ => true,
    }
}
pub fn effective_rules(s: &Settings) -> Result<FolderRules> {
    let mut r = load()?;
    let h = dirs::home_dir().context("HOME unavailable")?;
    let mut protect = |enabled: bool, rel: &str| { if !enabled { r.exclude.push(h.join(rel)); } };
    for (enabled, rel) in [
        (s.browsers.safari, "Library/Caches/com.apple.Safari"),
        (s.browsers.safari, "Library/Containers/com.apple.Safari/Data/Library/Caches"),
        (s.browsers.chrome, "Library/Caches/Google/Chrome"),
        (s.browsers.opera, "Library/Caches/com.operasoftware.Opera"),
        (s.browsers.firefox, "Library/Caches/Firefox"),
        (s.browsers.chromium, "Library/Caches/Chromium"),
        (s.browsers.brave, "Library/Caches/BraveSoftware"),
        (s.browsers.arc, "Library/Caches/company.thebrowser.Browser"),
        (s.development.pip, "Library/Caches/pip"),
        (s.development.uv, "Library/Caches/uv"),
        (s.development.yarn, "Library/Caches/Yarn"),
        (s.development.pnpm, "Library/Caches/pnpm"),
        (s.development.cocoapods, "Library/Caches/CocoaPods"),
        (s.macos.crash_reports, "Library/Logs/DiagnosticReports"),
        (s.homebrew.enabled && s.homebrew.cache, "Library/Caches/Homebrew"),
    ] { protect(enabled, rel); }
    // Browser profiles/session databases are never cleanup roots, even via custom rules.
    for rel in ["Library/Safari", "Library/Cookies", "Library/WebKit", "Library/Application Support/Google", "Library/Application Support/Firefox", "Library/Application Support/BraveSoftware", "Library/Application Support/Arc", "Library/Application Support/Chromium", "Library/Application Support/com.operasoftware.Opera", "Library/Containers/com.apple.Safari/Data/Library/Safari", "Library/Containers/com.apple.Safari/Data/Library/WebKit"] { r.exclude.push(h.join(rel)); }
    Ok(r)
}
pub fn mobile_enabled(s: &Settings) -> Result<bool> {
    let mobile = dirs::home_dir().context("HOME unavailable")?.join("Library/Application Support/MobileSync/Backup");
    Ok(s.mobile.delete_all_local_backups && !effective_rules(s)?.overlaps_exclusion(&mobile))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn exclusions_protect_ancestors_and_descendants() {
        let r = FolderRules { include: vec![], exclude: vec!["/tmp/cache/keep".into()] };
        assert!(r.overlaps_exclusion(Path::new("/tmp/cache")));
        assert!(r.overlaps_exclusion(Path::new("/tmp/cache/keep/item")));
        assert!(!r.overlaps_exclusion(Path::new("/tmp/cache/keeper")));
    }
    #[test] fn settings_switches_reach_engine() {
        let mut s = Settings::default(); s.development.cargo = false;
        assert!(!preset_enabled("Cargo sources", &s));
        s.macos.application_caches = false;
        assert!(!preset_enabled("Application caches", &s));
    }
    #[test] fn old_settings_get_safari_default() {
        let mut v = serde_json::to_value(Settings::default()).unwrap();
        v["browsers"].as_object_mut().unwrap().remove("safari");
        assert!(serde_json::from_value::<Settings>(v).unwrap().browsers.safari);
    }
    #[test] fn reject_home_and_secrets() {
        let h = dirs::home_dir().unwrap();
        assert!(validate_custom(&h).is_err());
        assert!(validate_custom(&h.join("Library")).is_err());
    }
}
