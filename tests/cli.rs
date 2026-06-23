use std::collections::BTreeMap;
use std::env;
use std::sync::Mutex;

use clap::Parser;

use ascii_animation::cli::{scene_from_run_args, Cli, Command};
use ascii_animation::presets::{build_default_registry, OptionValue};
use ascii_animation::render::ansi::render_to_ansi;
use ascii_animation::runtime::{prepare_scene_terminal, render_scene_frame, TerminalDriver};
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};

static HOME_LOCK: Mutex<()> = Mutex::new(());

fn galaxy_scene(color: bool) -> Scene {
    let mut options = BTreeMap::new();
    options.insert("arms".to_string(), OptionValue::Int(3));
    options.insert(
        "palette".to_string(),
        OptionValue::Choice("cosmic".to_string()),
    );

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

    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };
    let scene = scene_from_run_args(&args, &build_default_registry()).unwrap();

    assert!(!scene.color);
    assert_eq!(scene.instances.len(), 1);
    assert_eq!(scene.instances[0].preset, "galaxy");
    assert_eq!(
        scene.instances[0]
            .options
            .get("arms")
            .unwrap()
            .as_cli_value(),
        "4"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("stars")
            .unwrap()
            .as_cli_value(),
        "700"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("palette")
            .unwrap()
            .as_cli_value(),
        "mono"
    );
}

#[test]
fn leaves_seed_unset_when_flag_is_omitted() {
    let cli = Cli::parse_from(["ascii-animation", "run", "galaxy"]);
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

    assert!(format!("{args:?}").contains("seed: None"));
}

#[test]
fn parses_explicit_seed_value() {
    let cli = Cli::parse_from(["ascii-animation", "run", "galaxy", "--seed", "17"]);
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

    assert!(format!("{args:?}").contains("seed: Some(17)"));
}


#[test]
fn preserves_config_scene_color_without_no_color() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    galaxy_scene(false).save_to_path(&path).unwrap();

    let cli = Cli::parse_from(["ascii-animation", "run", "--config", path.to_str().unwrap()]);
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

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
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

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
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

    let err = scene_from_run_args(&args, &build_default_registry())
        .unwrap_err()
        .to_string();

    assert_eq!(err, "unknown scene: mystery");
}

#[test]
fn rejects_invalid_galaxy_option_range() {
    let cli = Cli::parse_from(["ascii-animation", "run", "galaxy", "--arms", "99"]);
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

    let err = scene_from_run_args(&args, &build_default_registry())
        .unwrap_err()
        .to_string();

    assert_eq!(
        err,
        "option `arms` is out of range: expected 1..=10, got 99"
    );
}

#[test]
fn rejects_direct_preset_flags_with_config_scene() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    galaxy_scene(true).save_to_path(&path).unwrap();

    let cli = Cli::parse_from([
        "ascii-animation",
        "run",
        "--config",
        path.to_str().unwrap(),
        "--arms",
        "4",
    ]);
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

    let err = scene_from_run_args(&args, &build_default_registry())
        .unwrap_err()
        .to_string();

    assert_eq!(
        err,
        "cannot combine --config with direct preset inputs: --arms"
    );
}

#[test]
fn rejects_direct_preset_name_with_default_scene() {
    let cli = Cli::parse_from(["ascii-animation", "run", "galaxy", "--scene", "default"]);
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

    let err = scene_from_run_args(&args, &build_default_registry())
        .unwrap_err()
        .to_string();

    assert_eq!(
        err,
        "cannot combine --scene with direct preset inputs: preset"
    );
}

#[test]
fn rejects_combining_config_and_scene_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    galaxy_scene(true).save_to_path(&path).unwrap();

    let cli = Cli::parse_from([
        "ascii-animation",
        "run",
        "--config",
        path.to_str().unwrap(),
        "--scene",
        "default",
    ]);
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };

    let err = scene_from_run_args(&args, &build_default_registry())
        .unwrap_err()
        .to_string();

    assert_eq!(err, "cannot combine --config with --scene");
}

#[derive(Debug)]
struct FillRenderer {
    ch: char,
}

impl ascii_animation::render::AnimationRenderer for FillRenderer {
    fn render(
        &mut self,
        frame: &mut ascii_animation::render::FrameBuffer,
        context: ascii_animation::render::RenderContext,
    ) {
        for y in 0..context.height {
            for x in 0..context.width {
                frame.put_cell(
                    context.x_offset + x,
                    context.y_offset + y,
                    ascii_animation::render::Cell::visible(
                        self.ch,
                        None,
                        context.layer,
                        context.z_index,
                        context.order,
                    ),
                );
            }
        }
    }
}

fn demo_renderer(
    _options: &BTreeMap<String, OptionValue>,
    _seed: u64,
) -> ascii_animation::Result<Box<dyn ascii_animation::render::AnimationRenderer>> {
    Ok(Box::new(FillRenderer { ch: '#' }))
}

#[test]
fn render_scene_frame_dispatches_registered_presets() {
    let registry = ascii_animation::presets::PresetRegistry::new(vec![
        ascii_animation::presets::PresetDescriptor::new(
            "demo",
            "Demo",
            "Test preset",
            vec![],
            demo_renderer,
        ),
    ]);
    let scene = Scene {
        frame_rate: 24,
        color: false,
        instances: vec![AnimationInstance {
            id: "demo-1".to_string(),
            preset: "demo".to_string(),
            options: BTreeMap::new(),
            placement: Placement::Fill,
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        }],
    };

    let frame = render_scene_frame(&scene, &registry, 1, 0.0, 6, 3).unwrap();

    assert_eq!(frame.get(0, 0).unwrap().ch, '#');
    assert_eq!(frame.get(5, 2).unwrap().ch, '#');
}


#[test]
fn direct_scene_renders_non_empty_frame() {
    let cli = Cli::parse_from([
        "ascii-animation",
        "run",
        "galaxy",
        "--stars",
        "100",
        "--no-color",
    ]);
    let Command::Run(args) = cli.command else {
        panic!("expected run command")
    };
    let registry = build_default_registry();
    let scene = scene_from_run_args(&args, &registry).unwrap();

    let frame = render_scene_frame(&scene, &registry, 1, 0.0, 40, 16).unwrap();
    let text = render_to_ansi(&frame, false);

    assert_eq!(text.lines().count(), 16);
    assert!(text.chars().any(|ch| ch != ' ' && ch != '\n'));
}

#[test]
fn render_scene_frame_respects_non_fill_placement() {
    let registry = build_default_registry();
    let mut scene = galaxy_scene(false);
    scene.instances[0].placement = Placement::Right;
    scene.instances[0]
        .options
        .insert("size".to_string(), OptionValue::Int(20));

    let frame = render_scene_frame(&scene, &registry, 1, 0.0, 40, 16).unwrap();

    assert!((0..20).all(|x| (0..16).all(|y| frame.get(x, y).unwrap().ch == ' ')));
    assert!((20..40).any(|x| (0..16).any(|y| frame.get(x, y).unwrap().ch != ' ')));
}

struct FailingTerminal {
    raw_enabled: bool,
    raw_disabled: bool,
    restored: bool,
}

impl TerminalDriver for FailingTerminal {
    fn enable_raw_mode(&mut self) -> std::io::Result<()> {
        self.raw_enabled = true;
        Ok(())
    }

    fn disable_raw_mode(&mut self) -> std::io::Result<()> {
        self.raw_disabled = true;
        Ok(())
    }

    fn setup_scene_terminal<W: std::io::Write>(&mut self, _stdout: &mut W) -> std::io::Result<()> {
        Err(std::io::Error::other("setup failed"))
    }

    fn restore_scene_terminal<W: std::io::Write>(
        &mut self,
        _stdout: &mut W,
    ) -> std::io::Result<()> {
        self.restored = true;
        Ok(())
    }

    fn poll(&mut self, _timeout: std::time::Duration) -> std::io::Result<bool> {
        Ok(false)
    }

    fn read(&mut self) -> std::io::Result<crossterm::event::Event> {
        unreachable!("read should not be called")
    }

    fn size(&mut self) -> std::io::Result<(u16, u16)> {
        Ok((40, 16))
    }
}

#[test]
fn prepare_scene_terminal_disables_raw_mode_when_setup_fails() {
    let mut terminal = FailingTerminal {
        raw_enabled: false,
        raw_disabled: false,
        restored: false,
    };
    let mut stdout = Vec::new();

    let err = prepare_scene_terminal(&mut stdout, &mut terminal).unwrap_err();

    assert_eq!(err.to_string(), "terminal error: setup failed");
    assert!(terminal.raw_enabled);
    assert!(terminal.raw_disabled);
    assert!(terminal.restored);
}
