use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::error::AsciiAnimError;
use crate::presets::{build_default_registry, OptionValue, PresetRegistry};
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
    Run(Box<RunArgs>),
    Tui(TuiArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    pub preset: Option<String>,

    #[arg(long)]
    pub scene: Option<String>,

    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long)]
    pub no_color: bool,

    #[arg(long)]
    pub arms: Option<i64>,

    #[arg(long)]
    pub stars: Option<i64>,

    #[arg(long)]
    pub speed: Option<i64>,

    #[arg(long)]
    pub size: Option<i64>,

    #[arg(long)]
    pub twist: Option<f64>,

    #[arg(long)]
    pub noise: Option<f64>,

    #[arg(long)]
    pub glow: Option<f64>,

    #[arg(long)]
    pub twinkle: Option<f64>,

    #[arg(long)]
    pub palette: Option<String>,

    #[arg(long)]
    pub gradient: Option<String>,

    #[arg(long, default_value_t = 0)]
    pub seed: u64,
}

#[derive(Debug, Args)]
pub struct TuiArgs {}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let registry = build_default_registry();
    match cli.command {
        Command::Run(args) => {
            let scene = scene_from_run_args(&args, &registry)?;
            runtime::run_scene(scene, &registry, args.seed)?;
        }
        Command::Tui(_) => tui::run(&registry)?,
    }
    Ok(())
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

fn direct_preset_inputs(args: &RunArgs) -> Vec<&'static str> {
    let mut inputs = Vec::new();
    if args.preset.is_some() {
        inputs.push("preset");
    }
    push_flag(&mut inputs, "--arms", args.arms.is_some());
    push_flag(&mut inputs, "--stars", args.stars.is_some());
    push_flag(&mut inputs, "--speed", args.speed.is_some());
    push_flag(&mut inputs, "--size", args.size.is_some());
    push_flag(&mut inputs, "--twist", args.twist.is_some());
    push_flag(&mut inputs, "--noise", args.noise.is_some());
    push_flag(&mut inputs, "--glow", args.glow.is_some());
    push_flag(&mut inputs, "--twinkle", args.twinkle.is_some());
    push_flag(&mut inputs, "--palette", args.palette.is_some());
    push_flag(&mut inputs, "--gradient", args.gradient.is_some());
    inputs
}

fn push_flag(inputs: &mut Vec<&'static str>, flag: &'static str, enabled: bool) {
    if enabled {
        inputs.push(flag);
    }
}

pub fn scene_from_run_args(args: &RunArgs, registry: &PresetRegistry) -> Result<Scene> {
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
    let mut raw = BTreeMap::new();

    insert_int(&mut raw, "arms", args.arms);
    insert_int(&mut raw, "stars", args.stars);
    insert_int(&mut raw, "speed", args.speed);
    insert_int(&mut raw, "size", args.size);
    insert_float(&mut raw, "twist", args.twist);
    insert_float(&mut raw, "noise", args.noise);
    insert_float(&mut raw, "glow", args.glow);
    insert_float(&mut raw, "twinkle", args.twinkle);
    insert_choice(&mut raw, "palette", args.palette.clone());
    insert_choice(&mut raw, "gradient", args.gradient.clone());

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

fn insert_int(raw: &mut BTreeMap<String, OptionValue>, name: &str, value: Option<i64>) {
    if let Some(value) = value {
        raw.insert(name.to_string(), OptionValue::Int(value));
    }
}

fn insert_float(raw: &mut BTreeMap<String, OptionValue>, name: &str, value: Option<f64>) {
    if let Some(value) = value {
        raw.insert(name.to_string(), OptionValue::Float(value));
    }
}

fn insert_choice(raw: &mut BTreeMap<String, OptionValue>, name: &str, value: Option<String>) {
    if let Some(value) = value {
        raw.insert(name.to_string(), OptionValue::Choice(value));
    }
}
