use std::collections::{BTreeMap, BTreeSet};

use crate::render::AnimationRenderer;
use crate::{AsciiAnimError, Result};

pub mod galaxy;
pub mod text_art;
pub mod confetti;
pub mod fire;
pub mod matrix;
mod palette;
pub mod plasma;
pub mod starfield;

pub type RendererFactory =
    fn(&BTreeMap<String, OptionValue>, u64) -> Result<Box<dyn AnimationRenderer>>;

pub type LogicalWidthHintFactory = fn(&BTreeMap<String, OptionValue>) -> Result<Option<u16>>;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "value")]
pub enum OptionValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Choice(String),
    Text(String),
}

impl OptionValue {
    pub fn as_cli_value(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::Float(value) => trim_float(*value),
            Self::Bool(value) => value.to_string(),
            Self::Choice(value) => value.clone(),
            Self::Text(value) => value.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptionKind {
    Int { min: i64, max: i64, step: i64 },
    Float { min: f64, max: f64 },
    Bool,
    Choice { choices: Vec<String> },
    Text { max_len: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionGroup {
    Basics,
    Motion,
    Style,
    Layout,
}

#[derive(Debug, Clone)]
pub struct OptionDescriptor {
    name: String,
    label: String,
    default: OptionValue,
    kind: OptionKind,
    rebuilds_state: bool,
    group: OptionGroup,
    help: String,
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
        Self::int_step(name, label, default, min, max, 1, rebuilds_state)
    }

    pub fn int_step(
        name: &str,
        label: &str,
        default: i64,
        min: i64,
        max: i64,
        step: i64,
        rebuilds_state: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            default: OptionValue::Int(default),
            kind: OptionKind::Int { min, max, step },
            rebuilds_state,
            group: legacy_group(name),
            help: legacy_help(name).to_string(),
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
            group: legacy_group(name),
            help: legacy_help(name).to_string(),
        }
    }

    pub fn bool(name: &str, label: &str, default: bool, rebuilds_state: bool) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            default: OptionValue::Bool(default),
            kind: OptionKind::Bool,
            rebuilds_state,
            group: legacy_group(name),
            help: legacy_help(name).to_string(),
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
            group: legacy_group(name),
            help: legacy_help(name).to_string(),
        }
    }

    pub fn text(
        name: &str,
        label: &str,
        default: &str,
        max_len: usize,
        rebuilds_state: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            default: OptionValue::Text(default.to_string()),
            kind: OptionKind::Text { max_len },
            rebuilds_state,
            group: legacy_group(name),
            help: legacy_help(name).to_string(),
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

    pub fn group(&self) -> OptionGroup {
        self.group
    }

    pub fn help(&self) -> &str {
        &self.help
    }

    pub fn with_group(mut self, group: OptionGroup) -> Self {
        self.group = group;
        self
    }

    pub fn with_help(mut self, help: &str) -> Self {
        self.help = help.to_string();
        self
    }

    fn validate(&self, preset: &str, value: OptionValue) -> Result<OptionValue> {
        match (&self.kind, value) {
            (OptionKind::Int { min, max, step }, OptionValue::Int(value))
                if value >= *min && value <= *max && ((value - *min) % *step == 0) =>
            {
                Ok(OptionValue::Int(value))
            }
            (OptionKind::Int { min, max, .. }, OptionValue::Int(value)) => {
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
            (OptionKind::Text { max_len }, OptionValue::Text(value))
                if value.chars().count() <= *max_len
                    && value.chars().all(|ch| ch.is_ascii_graphic() || ch == ' ') =>
            {
                Ok(OptionValue::Text(value))
            }
            (OptionKind::Text { max_len }, OptionValue::Text(value))
                if value.chars().count() > *max_len =>
            {
                Err(AsciiAnimError::TextTooLong {
                    option: self.name.clone(),
                    max: *max_len,
                    actual: value.chars().count(),
                })
            }
            (OptionKind::Text { .. }, OptionValue::Text(value)) => {
                Err(AsciiAnimError::InvalidOptionType {
                    option: self.name.clone(),
                    expected: "ASCII text",
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
            Self::Text { .. } => "text",
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
    logical_width_hint_factory: Option<LogicalWidthHintFactory>,
    option_visibility: Option<fn(&str, &BTreeMap<String, OptionValue>) -> bool>,
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
            logical_width_hint_factory: None,
            option_visibility: None,
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

    pub fn with_option_visibility(
        mut self,
        visibility: fn(&str, &BTreeMap<String, OptionValue>) -> bool,
    ) -> Self {
        self.option_visibility = Some(visibility);
        self
    }

    pub fn visible_options(
        &self,
        options: &BTreeMap<String, OptionValue>,
    ) -> Vec<&OptionDescriptor> {
        self.options
            .iter()
            .filter(|option| self.option_visibility.map_or(true, |visible| visible(option.name(), options)))
            .collect()
    }

    pub fn create_renderer(
        &self,
        options: &BTreeMap<String, OptionValue>,
        seed: u64,
    ) -> Result<Box<dyn AnimationRenderer>> {
        (self.renderer_factory)(options, seed)
    }

    pub fn with_logical_width_hint(mut self, factory: LogicalWidthHintFactory) -> Self {
        self.logical_width_hint_factory = Some(factory);
        self
    }

    pub fn logical_width_hint(
        &self,
        options: &BTreeMap<String, OptionValue>,
    ) -> Result<Option<u16>> {
        match self.logical_width_hint_factory {
            Some(factory) => factory(options),
            None => Ok(None),
        }
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

    pub fn descriptors(&self) -> impl Iterator<Item = &PresetDescriptor> {
        self.presets.values()
    }
}

impl Default for PresetRegistry {
    fn default() -> Self {
        Self::new(vec![
            galaxy::descriptor(),
            text_art::descriptor(),
            matrix::descriptor(),
            starfield::descriptor(),
            plasma::descriptor(),
            fire::descriptor(),
            confetti::descriptor(),
        ])
    }
}

pub fn build_default_registry() -> PresetRegistry {
    PresetRegistry::default()
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

fn legacy_group(name: &str) -> OptionGroup {
    match name {
        "speed" | "twinkle" => OptionGroup::Motion,
        "glow" | "palette" | "gradient" => OptionGroup::Style,
        "size" => OptionGroup::Layout,
        _ => OptionGroup::Basics,
    }
}

fn legacy_help(name: &str) -> &'static str {
    match name {
        "arms" => "Number of spiral arms; changing this restarts this Animation instance.",
        "stars" => "Seeded star population; changing this restarts this Animation instance.",
        "speed" => "Rotation speed in degrees per second.",
        "size" => "Galaxy radius as a percentage of its Placement.",
        "twist" => "Spiral winding from the center toward the outer stars.",
        "noise" => "Seeded deviation from the spiral arms.",
        "glow" => "Boost the visible density of star glyphs.",
        "twinkle" => "Amount of periodic star brightness variation.",
        "palette" => "Star colors; glyph density remains readable without color.",
        "gradient" => "Glyph ramp used for star brightness.",
        _ => "",
    }
}
