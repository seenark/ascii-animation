use std::collections::BTreeMap;
use std::env;
use std::sync::Mutex;

use clap::Parser;

use ascii_animation::cli::{scene_from_run_args, Cli, Command};
use ascii_animation::presets::{build_default_registry, OptionValue};
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};

static HOME_LOCK: Mutex<()> = Mutex::new(());

fn galaxy_scene(color: bool) -> Scene {
    let mut options = BTreeMap::new();
    options.insert("arms".to_string(), OptionValue::Int(3));
    options.insert("palette".to_string(), OptionValue::Choice("cosmic".to_string()));

    Scene {
        frame_rate: 24,
        color,
        instances: vec![AnimationInstance {
            id: "galaxy-1".to_string(),
            preset: "galaxy".to_string(),
            options,
            placement: Placement::Center,
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        }],
    }
}

#[test]
fn parses_direct_galaxy_command() {
    let cli = Cli::parse_from([
        "ascii-animation",
        "run",
        "galaxy",
        "--arms",
        "4",
        "--stars",
        "700",
        "--palette",
        "mono",
        "--no-color",
    ]);

    let Command::Run(args) = cli.command else { panic!("expected run command") };
    let scene = scene_from_run_args(&args, &build_default_registry()).unwrap();

    assert!(!scene.color);
    assert_eq!(scene.instances.len(), 1);
    assert_eq!(scene.instances[0].preset, "galaxy");
    assert_eq!(scene.instances[0].options.get("arms").unwrap().as_cli_value(), "4");
    assert_eq!(scene.instances[0].options.get("stars").unwrap().as_cli_value(), "700");
    assert_eq!(scene.instances[0].options.get("palette").unwrap().as_cli_value(), "mono");
}

#[test]
fn preserves_config_scene_color_without_no_color() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    galaxy_scene(false).save_to_path(&path).unwrap();

    let cli = Cli::parse_from([
        "ascii-animation",
        "run",
        "--config",
        path.to_str().unwrap(),
    ]);
    let Command::Run(args) = cli.command else { panic!("expected run command") };

    let scene = scene_from_run_args(&args, &build_default_registry()).unwrap();

    assert!(!scene.color);
}

#[test]
fn preserves_default_scene_color_without_no_color() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    galaxy_scene(false).save_to_path(&path).unwrap();

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let cli = Cli::parse_from(["ascii-animation", "run", "--scene", "default"]);
    let Command::Run(args) = cli.command else { panic!("expected run command") };

    let scene = scene_from_run_args(&args, &build_default_registry()).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert!(!scene.color);
}

#[test]
fn rejects_unknown_scene_name() {
    let cli = Cli::parse_from(["ascii-animation", "run", "--scene", "mystery"]);
    let Command::Run(args) = cli.command else { panic!("expected run command") };

    let err = scene_from_run_args(&args, &build_default_registry()).unwrap_err().to_string();

    assert_eq!(err, "unknown scene: mystery");
}

#[test]
fn rejects_invalid_galaxy_option_range() {
    let cli = Cli::parse_from(["ascii-animation", "run", "galaxy", "--arms", "99"]);
    let Command::Run(args) = cli.command else { panic!("expected run command") };

    let err = scene_from_run_args(&args, &build_default_registry()).unwrap_err().to_string();

    assert_eq!(err, "option `arms` is out of range: expected 1..=10, got 99");
}
