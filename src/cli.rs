use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::PathBuf;

use clap::builder::PossibleValuesParser;
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::error::AsciiAnimError;
use crate::presets::{
    build_default_registry, OptionDescriptor, OptionKind, OptionValue, PresetDescriptor,
    PresetRegistry,
};
use crate::runtime;
use crate::scene::{AnimationInstance, Layer, Placement, Scene};
use crate::tui;
use crate::Result;

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

#[derive(Debug)]
enum ParsedCommand {
    Run(RunArgs),
    Tui,
}

pub fn run_command_for(registry: &PresetRegistry) -> ClapCommand {
    let mut command = ClapCommand::new("run")
        .about("Run a preset directly or load a saved scene")
        .arg(Arg::new("preset").value_name("PRESET"))
        .arg(Arg::new("scene").long("scene").value_name("scene"))
        .arg(
            Arg::new("config")
                .long("config")
                .value_name("config")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("no-color")
                .long("no-color")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("seed")
                .long("seed")
                .value_name("seed")
                .value_parser(value_parser!(u64)),
        );
    let mut seen = BTreeSet::new();
    for descriptor in registry.descriptors() {
        for option in descriptor.options() {
            if seen.insert(option.name().to_string()) {
                command = command.arg(descriptor_arg(option));
            }
        }
    }
    command
}

fn cli_command_for(registry: &PresetRegistry) -> ClapCommand {
    ClapCommand::new("ascii-animation")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Run preset ASCII animations in the terminal")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(run_command_for(registry))
        .subcommand(ClapCommand::new("tui").about("Open the interactive scene editor"))
}

fn descriptor_arg(option: &OptionDescriptor) -> Arg {
    let name = leaked(option.name());
    let arg = Arg::new(name).long(name).value_name(name);
    match option.kind() {
        OptionKind::Int { .. } => arg.value_parser(value_parser!(i64)),
        OptionKind::Float { .. } => arg.value_parser(value_parser!(f64)),
        OptionKind::Bool => arg.value_parser(value_parser!(bool)),
        OptionKind::Choice { choices } => arg.value_parser(PossibleValuesParser::new(
            choices
                .iter()
                .map(|choice| leaked(choice))
                .collect::<Vec<_>>(),
        )),
    }
}

fn leaked(value: &str) -> &'static str {
    Box::leak(value.to_string().into_boxed_str())
}

fn parse_command_from<I, T>(args: I, registry: &PresetRegistry) -> anyhow::Result<ParsedCommand>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = cli_command_for(registry).try_get_matches_from(args)?;
    match matches.subcommand() {
        Some(("run", run_matches)) => Ok(ParsedCommand::Run(parse_run_matches(run_matches, registry))),
        Some(("tui", _)) => Ok(ParsedCommand::Tui),
        _ => anyhow::bail!("expected subcommand"),
    }
}

fn parse_run_matches(matches: &ArgMatches, registry: &PresetRegistry) -> RunArgs {
    let mut parsed = RunArgs {
        preset: matches.get_one::<String>("preset").cloned(),
        scene: matches.get_one::<String>("scene").cloned(),
        config: matches.get_one::<PathBuf>("config").cloned(),
        no_color: matches.get_flag("no-color"),
        seed: matches.get_one::<u64>("seed").copied(),
        ..RunArgs::default()
    };
    if parsed.preset.is_some() {
        parsed.direct_inputs.push("preset".to_string());
    }

    let mut seen = BTreeSet::new();
    for descriptor in registry.descriptors() {
        for option in descriptor.options() {
            if !seen.insert(option.name().to_string()) {
                continue;
            }
            if let Some(value) = descriptor_value_from_matches(matches, option) {
                parsed.direct_inputs.push(format!("--{}", option.name()));
                parsed.direct_options.insert(option.name().to_string(), value);
            }
        }
    }

    parsed
}

fn descriptor_value_from_matches(matches: &ArgMatches, option: &OptionDescriptor) -> Option<String> {
    match option.kind() {
        OptionKind::Int { .. } => matches.get_one::<i64>(option.name()).map(ToString::to_string),
        OptionKind::Float { .. } => matches.get_one::<f64>(option.name()).map(ToString::to_string),
        OptionKind::Bool => matches.get_one::<bool>(option.name()).map(ToString::to_string),
        OptionKind::Choice { .. } => matches.get_one::<String>(option.name()).cloned(),
    }
}

pub fn run() -> anyhow::Result<()> {
    let registry = build_default_registry();
    match parse_command_from(std::env::args_os(), &registry)? {
        ParsedCommand::Run(args) => {
            let scene = scene_from_run_args(&args, &registry)?;
            runtime::run_scene(scene, &registry, resolved_seed(args.seed))?;
        }
        ParsedCommand::Tui => tui::run(&registry)?,
    }
    Ok(())
}

pub fn parse_run_args_from<I, T>(args: I, registry: &PresetRegistry) -> anyhow::Result<RunArgs>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match parse_command_from(args, registry)? {
        ParsedCommand::Run(args) => Ok(args),
        ParsedCommand::Tui => anyhow::bail!("expected run command"),
    }
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
