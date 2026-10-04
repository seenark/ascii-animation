use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::presets::{OptionValue, PresetRegistry};
use crate::{AsciiAnimError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    Background,
    Normal,
    Foreground,
}

impl Layer {
    pub fn priority(self) -> i32 {
        match self {
            Self::Background => 0,
            Self::Normal => 1,
            Self::Foreground => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Placement {
    Center,
    Top,
    Bottom,
    Left,
    Right,
    Fill,
    Custom {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationInstance {
    pub id: String,
    pub preset: String,
    pub options: BTreeMap<String, OptionValue>,
    pub placement: Placement,
    pub layer: Layer,
    pub z_index: i32,
    pub enabled: bool,
}
impl AnimationInstance {
    fn supports_direct_run_export(&self) -> bool {
        self.enabled
            && self.placement == Placement::Center
            && self.layer == Layer::Normal
            && self.z_index == 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub frame_rate: u16,
    pub color: bool,
    pub instances: Vec<AnimationInstance>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            frame_rate: 30,
            color: true,
            instances: Vec::new(),
        }
    }
}

impl Scene {
    pub fn default_config_path() -> PathBuf {
        BaseDirs::new()
            .map(|dirs| dirs.home_dir().join(".config/ascii-animation/scene.toml"))
            .unwrap_or_else(|| PathBuf::from(".config/ascii-animation/scene.toml"))
    }

    pub fn load_default_config_if_available() -> Result<Option<Self>> {
        Self::load_default_config_raw_if_available()?.map(Self::validate).transpose()
    }

    pub fn load_default_config_raw_if_available() -> Result<Option<Self>> {
        let path = Self::default_config_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(AsciiAnimError::Terminal(source.to_string())),
        };
        Self::parse_config_text(&path, &text).map(Some)
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        Self::load_from_path_raw(path)?.validate()
    }

    pub fn load_from_path_raw(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|source| AsciiAnimError::Terminal(source.to_string()))?;
        Self::parse_config_text(path, &text)
    }

    fn parse_config_text(path: &Path, text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|source| AsciiAnimError::SceneConfigParse {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);
        let text = toml::to_string_pretty(self)
            .map_err(|source| AsciiAnimError::Terminal(source.to_string()))?;
        let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let save = || -> std::io::Result<()> {
            std::fs::create_dir_all(parent)?;
            let filename = path.file_name().ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidInput, "Scene path must name a file",
            ))?;
            let mut temporary_name = std::ffi::OsString::from(".");
            temporary_name.push(filename);
            temporary_name.push(format!(".{}.{}.tmp", std::process::id(), NEXT_WRITE.fetch_add(1, Ordering::Relaxed)));
            let temporary = parent.join(temporary_name);
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            let result = (|| {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if metadata.is_file() {
                        file.set_permissions(metadata.permissions())?;
                    }
                }
                file.write_all(text.as_bytes())?;
                file.sync_all()?;
                drop(file);
                std::fs::rename(&temporary, path)
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&temporary);
            }
            result
        };
        save().map_err(|source| AsciiAnimError::SceneConfigWrite {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn requires_config_export(&self) -> bool {
        !(self.frame_rate == 30
            && self.instances.len() == 1
            && self.instances[0].supports_direct_run_export())
    }

    pub fn export_command(&self) -> String {
        if self.requires_config_export() {
            "ascii-animation run --config ~/.config/ascii-animation/scene.toml".to_string()
        } else {
            self.direct_run_export_command()
        }
    }

fn shell_quote_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

    fn direct_run_export_command(&self) -> String {
        let instance = &self.instances[0];
        let mut command = format!("ascii-animation run {}", instance.preset);
        for (name, value) in &instance.options {
            command.push_str(&format!(" --{} {}", name, Self::shell_quote_arg(&value.as_cli_value())));
        }
        if !self.color {
            command.push_str(" --no-color");
        }
        command
    }

    fn validate(self) -> Result<Self> {
        if self.instances.is_empty() {
            return Err(AsciiAnimError::EmptyScene);
        }

        let registry = PresetRegistry::default();
        for instance in &self.instances {
            let descriptor = registry.get(&instance.preset)?;
            descriptor.validate_options(&instance.options)?;
        }
        Ok(self)
    }
}
