use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::presets::OptionValue;

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
