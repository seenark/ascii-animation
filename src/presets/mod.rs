use std::collections::{BTreeMap, BTreeSet};

use crate::render::AnimationRenderer;
use crate::{AsciiAnimError, Result};

pub mod galaxy;

pub type RendererFactory =
    fn(&BTreeMap<String, OptionValue>, u64) -> Result<Box<dyn AnimationRenderer>>;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "value")]
pub enum OptionValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Choice(String),
}

impl OptionValue {
    pub fn as_cli_value(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::Float(value) => trim_float(*value),
            Self::Bool(value) => value.to_string(),
            Self::Choice(value) => value.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptionKind {
    Int { min: i64, max: i64 },
    Float { min: f64, max: f64 },
    Bool,
    Choice { choices: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct OptionDescriptor {
    name: String,
    label: String,
    default: OptionValue,
    kind: OptionKind,
    rebuilds_state: bool,
}

impl OptionDescriptor {
    pub fn int(
        name: &str,
        label: &str,
        default: i64,
        min: i64,
        max: i64,
        rebuilds_state: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            default: OptionValue::Int(default),
            kind: OptionKind::Int { min, max },
            rebuilds_state,
        }
    }

    pub fn float(
        name: &str,
        label: &str,
        default: f64,
        min: f64,
        max: f64,
        rebuilds_state: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            default: OptionValue::Float(default),
            kind: OptionKind::Float { min, max },
            rebuilds_state,
        }
    }

    pub fn bool(name: &str, label: &str, default: bool, rebuilds_state: bool) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            default: OptionValue::Bool(default),
            kind: OptionKind::Bool,
            rebuilds_state,
        }
    }

    pub fn choice(
        name: &str,
        label: &str,
        default: &str,
        choices: Vec<&str>,
        rebuilds_state: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            default: OptionValue::Choice(default.to_string()),
            kind: OptionKind::Choice {
                choices: choices.into_iter().map(str::to_string).collect(),
            },
            rebuilds_state,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn default(&self) -> &OptionValue {
        &self.default
    }
    pub fn kind(&self) -> &OptionKind {
        &self.kind
    }
    pub fn rebuilds_state(&self) -> bool {
        self.rebuilds_state
    }

    fn validate(&self, preset: &str, value: OptionValue) -> Result<OptionValue> {
        match (&self.kind, value) {
            (OptionKind::Int { min, max }, OptionValue::Int(value))
                if value >= *min && value <= *max =>
            {
                Ok(OptionValue::Int(value))
            }
            (OptionKind::Int { min, max }, OptionValue::Int(value)) => {
                Err(AsciiAnimError::OptionOutOfRange {
                    option: self.name.clone(),
                    min: min.to_string(),
                    max: max.to_string(),
                    actual: value.to_string(),
                })
            }
            (OptionKind::Float { min, max }, OptionValue::Float(value))
                if value >= *min && value <= *max =>
            {
                Ok(OptionValue::Float(value))
            }
            (OptionKind::Float { min, max }, OptionValue::Float(value)) => {
                Err(AsciiAnimError::OptionOutOfRange {
                    option: self.name.clone(),
                    min: trim_float(*min),
                    max: trim_float(*max),
                    actual: trim_float(value),
                })
            }
            (OptionKind::Bool, OptionValue::Bool(value)) => Ok(OptionValue::Bool(value)),
            (OptionKind::Choice { choices }, OptionValue::Choice(value))
                if choices.contains(&value) =>
            {
                Ok(OptionValue::Choice(value))
            }
            (OptionKind::Choice { choices }, OptionValue::Choice(value)) => {
                Err(AsciiAnimError::InvalidChoice {
                    option: self.name.clone(),
                    choices: choices.clone(),
                    actual: value,
                })
            }
            (kind, value) => Err(AsciiAnimError::InvalidOptionType {
                option: self.name.clone(),
                expected: kind.expected_name(),
                actual: value.as_cli_value(),
            }),
        }
        .map_err(|err| match err {
            AsciiAnimError::UnknownOption { .. } => err,
            other => {
                let _ = preset;
                other
            }
        })
    }
}

impl OptionKind {
    fn expected_name(&self) -> &'static str {
        match self {
            Self::Int { .. } => "integer",
            Self::Float { .. } => "float",
            Self::Bool => "bool",
            Self::Choice { .. } => "choice",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PresetDescriptor {
    name: String,
    label: String,
    description: String,
    options: Vec<OptionDescriptor>,
    renderer_factory: RendererFactory,
}

impl PresetDescriptor {
    pub fn new(
        name: &str,
        label: &str,
        description: &str,
        options: Vec<OptionDescriptor>,
        renderer_factory: RendererFactory,
    ) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            description: description.to_string(),
            options,
            renderer_factory,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn options(&self) -> &[OptionDescriptor] {
        &self.options
    }

    pub fn create_renderer(
        &self,
        options: &BTreeMap<String, OptionValue>,
        seed: u64,
    ) -> Result<Box<dyn AnimationRenderer>> {
        (self.renderer_factory)(options, seed)
    }

    pub fn defaults(&self) -> BTreeMap<String, OptionValue> {
        self.options
            .iter()
            .map(|option| (option.name.clone(), option.default.clone()))
            .collect()
    }

    pub fn validate_options(
        &self,
        raw: &BTreeMap<String, OptionValue>,
    ) -> Result<BTreeMap<String, OptionValue>> {
        let known: BTreeSet<&str> = self.options.iter().map(|option| option.name()).collect();
        for key in raw.keys() {
            if !known.contains(key.as_str()) {
                return Err(AsciiAnimError::UnknownOption {
                    preset: self.name.clone(),
                    option: key.clone(),
                });
            }
        }

        let mut values = BTreeMap::new();
        for option in &self.options {
            let value = raw
                .get(option.name())
                .cloned()
                .unwrap_or_else(|| option.default().clone());
            values.insert(option.name.clone(), option.validate(&self.name, value)?);
        }
        Ok(values)
    }
}

#[derive(Debug, Clone)]
pub struct PresetRegistry {
    presets: BTreeMap<String, PresetDescriptor>,
}

impl PresetRegistry {
    pub fn new(presets: Vec<PresetDescriptor>) -> Self {
        Self {
            presets: presets
                .into_iter()
                .map(|preset| (preset.name.clone(), preset))
                .collect(),
        }
    }

    pub fn get(&self, name: &str) -> Result<&PresetDescriptor> {
        self.presets
            .get(name)
            .ok_or_else(|| AsciiAnimError::UnknownPreset {
                name: name.to_string(),
            })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.presets.keys().map(String::as_str)
    }
}

impl Default for PresetRegistry {
    fn default() -> Self {
        Self::new(vec![galaxy::descriptor()])
    }
}

pub fn build_default_registry() -> PresetRegistry {
    PresetRegistry::new(vec![galaxy::descriptor()])
}

fn trim_float(value: f64) -> String {
    let mut text = value.to_string();
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}
