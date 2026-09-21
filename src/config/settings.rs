use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub macos: MacOsSettings,
    pub browsers: BrowserSettings,
    pub development: DevelopmentSettings,
    pub ai_ml: AiSettings,
    pub homebrew: HomebrewSettings,
    pub docker: DockerSettings,
    pub podman: PodmanSettings,
    pub mobile: MobileSettings,
    pub behavior: BehaviorSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacOsSettings {
    pub trash: bool,
    pub application_caches: bool,
    pub temporary_files: bool,
    pub temporary_min_age_days: u64,
    pub logs: bool,
    pub logs_min_age_days: u64,
    pub crash_reports: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSettings {
    pub chrome: bool,
    pub opera: bool,
    pub firefox: bool,
    pub chromium: bool,
    pub brave: bool,
    pub arc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentSettings {
    pub xcode_derived_data: bool,
    pub xcode_source_packages: bool,
    pub xcode_source_packages_min_age_days: u64,
    pub unavailable_simulators: bool,

    pub cargo: bool,
    pub npm: bool,
    pub npx: bool,
    pub pnpm: bool,
    pub yarn: bool,
    pub pip: bool,
    pub uv: bool,
    pub gradle: bool,
    pub gradle_min_age_days: u64,
    pub cocoapods: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub huggingface: bool,
    pub torch: bool,
    pub whisper: bool,
    pub clip: bool,
    pub coreml: bool,
    pub codex_runtimes: bool,
    pub selenium: bool,
    pub openai_python: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomebrewSettings {
    pub enabled: bool,
    pub autoremove: bool,
    pub old_versions: bool,
    pub cache: bool,
    pub temporary_builds: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerSettings {
    pub enabled: bool,
    pub build_cache: bool,
    pub unused_images: bool,
    pub stopped_containers: bool,
    pub unused_networks: bool,

    // Deliberately cannot be enabled.
    pub volumes_protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodmanSettings {
    pub enabled: bool,
    pub cache: bool,
    pub stale_machines: bool,
    pub stale_machine_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileSettings {
    // User policy: all local iPhone/iPad backups may be removed.
    pub delete_all_local_backups: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSettings {
    pub rescan_after_cleanup: bool,
    pub show_reclaimed_space: bool,
    pub write_log: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            macos: MacOsSettings {
                trash: true,
                application_caches: true,
                temporary_files: true,
                temporary_min_age_days: 2,
                logs: true,
                logs_min_age_days: 2,
                crash_reports: true,
            },

            browsers: BrowserSettings {
                chrome: true,
                opera: true,
                firefox: true,
                chromium: true,
                brave: true,
                arc: true,
            },

            development: DevelopmentSettings {
                xcode_derived_data: true,
                xcode_source_packages: true,
                xcode_source_packages_min_age_days: 7,
                unavailable_simulators: true,

                cargo: true,
                npm: true,
                npx: true,
                pnpm: true,
                yarn: true,
                pip: true,
                uv: true,
                gradle: true,
                gradle_min_age_days: 14,
                cocoapods: true,
            },

            ai_ml: AiSettings {
                huggingface: true,
                torch: true,
                whisper: true,
                clip: true,
                coreml: true,
                codex_runtimes: true,
                selenium: true,
                openai_python: true,
            },

            homebrew: HomebrewSettings {
                enabled: true,
                autoremove: true,
                old_versions: true,
                cache: true,
                temporary_builds: true,
            },

            docker: DockerSettings {
                enabled: true,
                build_cache: true,
                unused_images: true,
                stopped_containers: true,
                unused_networks: true,
                volumes_protected: true,
            },

            podman: PodmanSettings {
                enabled: true,
                cache: true,
                stale_machines: true,
                stale_machine_days: 90,
            },

            mobile: MobileSettings {
                delete_all_local_backups: true,
            },

            behavior: BehaviorSettings {
                rescan_after_cleanup: true,
                show_reclaimed_space: true,
                write_log: true,
            },
        }
    }
}
