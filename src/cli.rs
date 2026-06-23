use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::error::AsciiAnimError;
use crate::presets::{build_default_registry, OptionKind, OptionValue, PresetDescriptor, PresetRegistry};
use crate::runtime;
use crate::scene::{AnimationInstance, Layer, Placement, Scene};
use crate::tui;
use crate::Result;

#[derive(Debug, Parser)]
#[command(
    name = "ascii-animation",
    version,
    about = "Run preset ASCII animations in the terminal"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(Box<RawRunArgs>),
    Tui(TuiArgs),
}

#[derive(Debug, Args)]
pub struct RawRunArgs {
    #[arg(num_args = 0.., allow_hyphen_values = true)]
    pub raw: Vec<OsString>,
}

#[derive(Debug, Clone, Default)]
pub struct RunArgs {
    pub preset: Option<String>,
    pub scene: Option<String>,
    pub config: Option<PathBuf>,
    pub no_color: bool,
    pub seed: Option<u64>,
    direct_options: BTreeMap<String, String>,
    direct_inputs: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TuiArgs {}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let registry = build_default_registry();
    match cli.command {
        Command::Run(args) => {
            let args = parse_run_args(*args, &registry)?;
            let scene = scene_from_run_args(&args, &registry)?;
            runtime::run_scene(scene, &registry, resolved_seed(args.seed))?;
        }
        Command::Tui(_) => tui::run(&registry)?,
    }
    Ok(())
}

pub fn parse_run_args_from<I, T>(args: I, registry: &PresetRegistry) -> anyhow::Result<RunArgs>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args)?;
    match cli.command {
        Command::Run(args) => Ok(parse_run_args(*args, registry)?),
        Command::Tui(_) => anyhow::bail!("expected run command"),
    }
}

pub fn parse_run_args(args: RawRunArgs, _registry: &PresetRegistry) -> Result<RunArgs> {
    let mut parsed = RunArgs::default();
    let raw: Vec<String> = args
        .raw
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    let mut index = 0;

    while index < raw.len() {
        match raw[index].as_str() {
            "--scene" => {
                parsed.scene = Some(next_value(&raw, &mut index, "scene")?);
            }
            "--config" => {
                parsed.config = Some(PathBuf::from(next_value(&raw, &mut index, "config")?));
            }
            "--no-color" => {
                parsed.no_color = true;
                index += 1;
            }
            "--seed" => {
                let value = next_value(&raw, &mut index, "seed")?;
                parsed.seed = Some(parse_seed(&value)?);
            }
            token if token.starts_with("--") => {
                let name = token.trim_start_matches("--").to_string();
                let value = next_value(&raw, &mut index, &name)?;
                parsed.direct_inputs.push(token.to_string());
                parsed.direct_options.insert(name, value);
            }
            token => {
                if parsed.preset.is_some() {
                    return Err(AsciiAnimError::InvalidOptionType {
                        option: "preset".to_string(),
                        expected: "single preset name",
                        actual: token.to_string(),
                    });
                }
                parsed.preset = Some(token.to_string());
                parsed.direct_inputs.push("preset".to_string());
                index += 1;
            }
        }
    }

    Ok(parsed)
}

fn resolved_seed(seed: Option<u64>) -> u64 {
    resolve_seed_with(seed, rand::random::<u64>)
}

fn resolve_seed_with<F>(seed: Option<u64>, generate: F) -> u64
where
    F: FnOnce() -> u64,
{
    seed.unwrap_or_else(generate)
}

fn next_value(raw: &[String], index: &mut usize, option: &str) -> Result<String> {
    let value_index = *index + 1;
    let Some(value) = raw.get(value_index) else {
        return Err(AsciiAnimError::InvalidOptionType {
            option: option.to_string(),
            expected: "value",
            actual: "<missing>".to_string(),
        });
    };
    *index += 2;
    Ok(value.clone())
}

fn parse_seed(raw: &str) -> Result<u64> {
    raw.parse::<u64>().map_err(|_| AsciiAnimError::InvalidOptionType {
        option: "seed".to_string(),
        expected: "integer",
        actual: raw.to_string(),
    })
}

fn reject_conflicting_direct_inputs(args: &RunArgs, source: &'static str) -> Result<()> {
    let direct_inputs = direct_preset_inputs(args);
    if direct_inputs.is_empty() {
        return Ok(());
    }

    Err(AsciiAnimError::ConflictingRunInputs {
        input_source: source,
        conflicts: direct_inputs.join(", "),
    })
}

fn reject_conflicting_scene_inputs(args: &RunArgs) -> Result<()> {
    if args.config.is_some() && args.scene.is_some() {
        return Err(AsciiAnimError::ConflictingSceneInputs {
            left: "--config",
            right: "--scene",
        });
    }

    Ok(())
}

fn direct_preset_inputs(args: &RunArgs) -> Vec<String> {
    args.direct_inputs.clone()
}

pub fn scene_from_run_args(args: &RunArgs, registry: &PresetRegistry) -> Result<Scene> {
    reject_conflicting_scene_inputs(args)?;
    if let Some(path) = &args.config {
        reject_conflicting_direct_inputs(args, "--config")?;
        let mut scene = Scene::load_from_path(path)?;
        if args.no_color {
            scene.color = false;
        }
        return Ok(scene);
    }

    if let Some(scene_name) = args.scene.as_deref() {
        reject_conflicting_direct_inputs(args, "--scene")?;
        if scene_name == "default" {
            let mut scene = Scene::load_from_path(&Scene::default_config_path())?;
            if args.no_color {
                scene.color = false;
            }
            return Ok(scene);
        }

        return Err(AsciiAnimError::UnknownScene {
            name: scene_name.to_string(),
        });
    }

    let preset_name = args.preset.as_deref().unwrap_or("galaxy");
    let descriptor = registry.get(preset_name)?;
    let raw = descriptor_option_values(args, descriptor)?;
    let options = descriptor.validate_options(&raw)?;
    Ok(Scene {
        frame_rate: 30,
        color: !args.no_color,
        instances: vec![AnimationInstance {
            id: format!("{}-1", preset_name),
            preset: preset_name.to_string(),
            options,
            placement: Placement::Center,
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        }],
    })
}

fn descriptor_option_values(
    args: &RunArgs,
    descriptor: &PresetDescriptor,
) -> Result<BTreeMap<String, OptionValue>> {
    let mut raw = BTreeMap::new();

    for (name, value) in &args.direct_options {
        let option = descriptor
            .options()
            .iter()
            .find(|option| option.name() == name)
            .ok_or_else(|| AsciiAnimError::UnknownOption {
                preset: descriptor.name().to_string(),
                option: name.clone(),
            })?;
        raw.insert(name.clone(), parse_descriptor_value(option.kind(), name, value)?);
    }

    Ok(raw)
}

fn parse_descriptor_value(kind: &OptionKind, name: &str, raw: &str) -> Result<OptionValue> {
    match kind {
        OptionKind::Int { .. } => raw
            .parse::<i64>()
            .map(OptionValue::Int)
            .map_err(|_| AsciiAnimError::InvalidOptionType {
                option: name.to_string(),
                expected: "integer",
                actual: raw.to_string(),
            }),
        OptionKind::Float { .. } => raw
            .parse::<f64>()
            .map(OptionValue::Float)
            .map_err(|_| AsciiAnimError::InvalidOptionType {
                option: name.to_string(),
                expected: "float",
                actual: raw.to_string(),
            }),
        OptionKind::Bool => raw
            .parse::<bool>()
            .map(OptionValue::Bool)
            .map_err(|_| AsciiAnimError::InvalidOptionType {
                option: name.to_string(),
                expected: "bool",
                actual: raw.to_string(),
            }),
        OptionKind::Choice { .. } => Ok(OptionValue::Choice(raw.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_seed_with;

    #[test]
    fn explicit_seed_bypasses_generator() {
        let mut generated = false;
        let seed = resolve_seed_with(Some(17), || {
            generated = true;
            42
        });

        assert_eq!(seed, 17);
        assert!(!generated);
    }

    #[test]
    fn omitted_seed_uses_generator() {
        let seed = resolve_seed_with(None, || 42);

        assert_eq!(seed, 42);
    }
}
