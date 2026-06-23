use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::presets::OptionValue;
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
    Custom { x: u16, y: u16, width: u16, height: u16 },
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
        PathBuf::from("~/.config/ascii-animation/scene.toml")
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|source| AsciiAnimError::Terminal(source.to_string()))?;
        toml::from_str(&text).map_err(|source| AsciiAnimError::SceneConfigParse {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| AsciiAnimError::SceneConfigWrite {
                path: path.to_path_buf(),
                source,
            })?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|source| AsciiAnimError::Terminal(source.to_string()))?;
        std::fs::write(path, text).map_err(|source| AsciiAnimError::SceneConfigWrite {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn export_command(&self) -> String {
        let enabled: Vec<&AnimationInstance> = self
            .instances
            .iter()
            .filter(|instance| instance.enabled)
            .collect();
        if enabled.len() == 1 {
            let instance = enabled[0];
            let mut command = format!("ascii-animation run {}", instance.preset);
            for (name, value) in &instance.options {
                command.push_str(&format!(" --{} {}", name, value.as_cli_value()));
            }
            if !self.color {
                command.push_str(" --no-color");
            }
            command
        } else {
            "ascii-animation run --config ~/.config/ascii-animation/scene.toml".to_string()
        }
    }
}
