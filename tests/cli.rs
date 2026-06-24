use std::collections::BTreeMap;
use std::env;
use std::sync::Mutex;

use ascii_animation::cli::{parse_run_args_from, run_command_for, scene_from_run_args};
use ascii_animation::presets::{build_default_registry, OptionValue};
use ascii_animation::render::ansi::render_to_ansi;
use ascii_animation::runtime::{
    prepare_scene_terminal, render_centered_scene_frame, render_scene_frame, TerminalDriver,
};
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};

static HOME_LOCK: Mutex<()> = Mutex::new(());

static RECORDED_SEEDS: Mutex<Vec<u64>> = Mutex::new(Vec::new());

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


fn parse_run<I, S>(args: I) -> ascii_animation::cli::RunArgs
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    parse_run_args_from(args, &build_default_registry()).unwrap()
}

#[test]
fn parses_direct_galaxy_command() {
    let args = parse_run([
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
    let args = parse_run(["ascii-animation", "run", "galaxy"]);

    assert!(format!("{args:?}").contains("seed: None"));
}

#[test]
fn parses_explicit_seed_value() {
    let args = parse_run(["ascii-animation", "run", "galaxy", "--seed", "17"]);

    assert!(format!("{args:?}").contains("seed: Some(17)"));
}

#[test]
fn preserves_config_scene_color_without_no_color() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    galaxy_scene(false).save_to_path(&path).unwrap();

    let args = parse_run(["ascii-animation", "run", "--config", path.to_str().unwrap()]);
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

    let args = parse_run(["ascii-animation", "run", "--scene", "default"]);
    let scene = scene_from_run_args(&args, &build_default_registry()).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert!(!scene.color);
}

#[test]
fn bare_run_loads_saved_default_scene_when_present() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    galaxy_scene(false).save_to_path(&path).unwrap();

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let args = parse_run(["ascii-animation", "run"]);
    let scene = scene_from_run_args(&args, &build_default_registry()).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert!(!scene.color);
    assert_eq!(scene.frame_rate, 24);
    assert_eq!(scene.instances[0].preset, "galaxy");
}

#[test]
fn bare_run_falls_back_to_direct_galaxy_when_default_config_is_missing() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let args = parse_run(["ascii-animation", "run"]);
    let scene = scene_from_run_args(&args, &build_default_registry()).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert_eq!(scene.frame_rate, 30);
    assert!(scene.color);
    assert_eq!(scene.instances.len(), 1);
    assert_eq!(scene.instances[0].id, "galaxy-1");
}

#[test]
fn bare_run_no_color_overrides_saved_default_scene_color() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    galaxy_scene(true).save_to_path(&path).unwrap();

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let args = parse_run(["ascii-animation", "run", "--no-color"]);
    let scene = scene_from_run_args(&args, &build_default_registry()).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert!(!scene.color);
    assert_eq!(scene.frame_rate, 24);
}

#[test]
fn rejects_unknown_scene_name() {
    let args = parse_run(["ascii-animation", "run", "--scene", "mystery"]);

    let err = scene_from_run_args(&args, &build_default_registry())
        .unwrap_err()
        .to_string();

    assert_eq!(err, "unknown scene: mystery");
}

#[test]
fn rejects_invalid_galaxy_option_range() {
    let args = parse_run(["ascii-animation", "run", "galaxy", "--arms", "99"]);

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

    let args = parse_run([
        "ascii-animation",
        "run",
        "--config",
        path.to_str().unwrap(),
        "--arms",
        "4",
    ]);

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
    let args = parse_run(["ascii-animation", "run", "galaxy", "--scene", "default"]);

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

    let args = parse_run([
        "ascii-animation",
        "run",
        "--config",
        path.to_str().unwrap(),
        "--scene",
        "default",
    ]);

    let err = scene_from_run_args(&args, &build_default_registry())
        .unwrap_err()
        .to_string();

    assert_eq!(err, "cannot combine --config with --scene");
}

#[test]
fn parses_registered_preset_flags_from_descriptor_metadata() {
    let registry = ascii_animation::presets::PresetRegistry::new(vec![
        ascii_animation::presets::PresetDescriptor::new(
            "demo",
            "Demo",
            "Test preset",
            vec![
                ascii_animation::presets::OptionDescriptor::int_step(
                    "count", "Count", 3, 1, 9, 3, false,
                ),
                ascii_animation::presets::OptionDescriptor::choice(
                    "palette",
                    "Palette",
                    "mono",
                    vec!["mono", "cosmic"],
                    false,
                ),
                ascii_animation::presets::OptionDescriptor::text(
                    "message", "Message", "HELLO", 12, false,
                ),
            ],
            demo_renderer,
        ),
    ]);
    let args = ascii_animation::cli::parse_run_args_from(
        [
            "ascii-animation",
            "run",
            "demo",
            "--count",
            "7",
            "--palette",
            "cosmic",
            "--message",
            "HELLO",
        ],
        &registry,
    )
    .unwrap();
    let scene = scene_from_run_args(&args, &registry).unwrap();

    assert_eq!(scene.instances[0].preset, "demo");
    assert_eq!(
        scene.instances[0].options.get("count"),
        Some(&OptionValue::Int(7))
    );
    assert_eq!(
        scene.instances[0].options.get("palette"),
        Some(&OptionValue::Choice("cosmic".to_string()))
    );
    assert_eq!(
        scene.instances[0].options.get("message"),
        Some(&OptionValue::Text("HELLO".to_string()))
    );
}

#[test]
fn parses_direct_text_art_command() {
    let args = parse_run([
        "ascii-animation",
        "run",
        "text-art",
        "--text",
        "CODE",
        "--text-font",
        "dot-matrix",
        "--text-fill",
        "triangle",
        "--text-palette",
        "plasma",
        "--text-effect",
        "glitch",
        "--text-bg",
        "noise",
        "--text-overflow",
        "slide",
        "--text-speed",
        "2.5",
        "--text-scale",
        "1.2",
        "--text-drop-shadow",
        "false",
        "--text-block-shadow",
        "true",
        "--no-color",
    ]);
    let registry = build_default_registry();
    let scene = scene_from_run_args(&args, &registry).unwrap();

    assert!(!scene.color);
    assert_eq!(scene.instances[0].preset, "text-art");
    assert_eq!(scene.instances[0].id, "text-art-1");
    assert_eq!(
        scene.instances[0]
            .options
            .get("text")
            .unwrap()
            .as_cli_value(),
        "CODE"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-font")
            .unwrap()
            .as_cli_value(),
        "dot-matrix"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-fill")
            .unwrap()
            .as_cli_value(),
        "triangle"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-palette")
            .unwrap()
            .as_cli_value(),
        "plasma"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-effect")
            .unwrap()
            .as_cli_value(),
        "glitch"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-overflow")
            .unwrap()
            .as_cli_value(),
        "slide"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-bg")
            .unwrap()
            .as_cli_value(),
        "noise"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-speed")
            .unwrap()
            .as_cli_value(),
        "2.5"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-scale")
            .unwrap()
            .as_cli_value(),
        "1.2"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-drop-shadow")
            .unwrap()
            .as_cli_value(),
        "false"
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-block-shadow")
            .unwrap()
            .as_cli_value(),
        "true"
    );
}

#[test]
fn parses_dos_text_art_font_choice() {
    let args = parse_run([
        "ascii-animation",
        "run",
        "text-art",
        "--text",
        "OK",
        "--text-font",
        "dos",
        "--text-effect",
        "none",
        "--text-bg",
        "none",
        "--no-color",
    ]);
    let registry = build_default_registry();
    let scene = scene_from_run_args(&args, &registry).unwrap();

    assert_eq!(
        scene.instances[0]
            .options
            .get("text-font")
            .unwrap()
            .as_cli_value(),
        "dos"
    );
}

#[test]
fn parses_long_direct_text_art_command() {
    let text = "LONG TERMINAL TEXT";
    let args = parse_run([
        "ascii-animation",
        "run",
        "text-art",
        "--text",
        text,
        "--text-bg",
        "none",
    ]);
    let registry = build_default_registry();
    let scene = scene_from_run_args(&args, &registry).unwrap();

    assert_eq!(
        scene.instances[0]
            .options
            .get("text")
            .unwrap()
            .as_cli_value(),
        text
    );
    assert_eq!(
        scene.instances[0]
            .options
            .get("text-overflow")
            .unwrap()
            .as_cli_value(),
        "extend"
    );
}

#[test]
fn render_centered_scene_frame_extends_text_art_canvas_for_extend_overflow() {
    let registry = build_default_registry();
    let mut options = ascii_animation::presets::text_art::descriptor().defaults();
    options.insert(
        "text".to_string(),
        OptionValue::Text("LONG TERMINAL TEXT".to_string()),
    );
    options.insert(
        "text-bg".to_string(),
        OptionValue::Choice("none".to_string()),
    );
    options.insert(
        "text-effect".to_string(),
        OptionValue::Choice("wave".to_string()),
    );
    options.insert("text-amp".to_string(), OptionValue::Float(0.0));
    options.insert("text-glow".to_string(), OptionValue::Bool(false));
    options.insert("text-drop-shadow".to_string(), OptionValue::Bool(false));
    options.insert("text-block-shadow".to_string(), OptionValue::Bool(false));
    let options = registry
        .get("text-art")
        .unwrap()
        .validate_options(&options)
        .unwrap();
    let scene = Scene {
        frame_rate: 30,
        color: false,
        instances: vec![AnimationInstance {
            id: "text-art-1".to_string(),
            preset: "text-art".to_string(),
            options,
            placement: Placement::Center,
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        }],
    };

    let frame = render_centered_scene_frame(&scene, &registry, 7, 0.0, 124, 16).unwrap();
    let plain = render_to_ansi(&frame, false);

    assert_eq!(frame.width(), 124);
    assert!(plain.lines().any(|line| line.chars().any(|ch| ch != ' ')));
    assert!(
        plain.lines().any(|line| {
            line.char_indices()
                .rev()
                .find(|(_, ch)| *ch != ' ')
                .map(|(idx, _)| idx > 100)
                .unwrap_or(false)
        })
    );
}

#[test]
fn clap_help_lists_descriptor_defined_run_flags() {
    let registry = ascii_animation::presets::PresetRegistry::new(vec![
        ascii_animation::presets::PresetDescriptor::new(
            "demo",
            "Demo",
            "Test preset",
            vec![
                ascii_animation::presets::OptionDescriptor::int_step(
                    "count", "Count", 3, 1, 9, 3, false,
                ),
                ascii_animation::presets::OptionDescriptor::choice(
                    "palette",
                    "Palette",
                    "mono",
                    vec!["mono", "cosmic"],
                    false,
                ),
            ],
            demo_renderer,
        ),
    ]);
    let mut command = run_command_for(&registry);
    let mut help = Vec::new();
    command.write_long_help(&mut help).unwrap();
    let help = String::from_utf8(help).unwrap();

    assert!(help.contains("run"));
    assert!(help.contains("--count <count>"));
    assert!(help.contains("--palette <palette>"));
}

#[test]
fn clap_rejects_unknown_run_flags_after_descriptor_registration() {
    let registry = ascii_animation::presets::PresetRegistry::new(vec![
        ascii_animation::presets::PresetDescriptor::new(
            "demo",
            "Demo",
            "Test preset",
            vec![ascii_animation::presets::OptionDescriptor::int_step(
                "count", "Count", 3, 1, 9, 3, false,
            )],
            demo_renderer,
        ),
    ]);
    let err = parse_run_args_from(
        ["ascii-animation", "run", "demo", "--sparkles", "7"],
        &registry,
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("unexpected argument '--sparkles' found"));
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

fn seed_recording_renderer(
    _options: &BTreeMap<String, OptionValue>,
    seed: u64,
) -> ascii_animation::Result<Box<dyn ascii_animation::render::AnimationRenderer>> {
    RECORDED_SEEDS.lock().unwrap().push(seed);
    Ok(Box::new(FillRenderer { ch: '#' }))
}

fn assert_filled_region(
    frame: &ascii_animation::render::FrameBuffer,
    expected_x: u16,
    expected_y: u16,
    expected_width: u16,
    expected_height: u16,
) {
    for y in 0..frame.height() {
        for x in 0..frame.width() {
            let expected = x >= expected_x
                && x < expected_x + expected_width
                && y >= expected_y
                && y < expected_y + expected_height;
            assert_eq!(
                frame.get(x, y).unwrap().ch == '#',
                expected,
                "unexpected fill at ({x}, {y})",
            );
        }
    }
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
fn render_scene_frame_uses_distinct_bounds_for_non_fill_placements() {
    let registry = ascii_animation::presets::PresetRegistry::new(vec![
        ascii_animation::presets::PresetDescriptor::new(
            "demo",
            "Demo",
            "Test preset",
            vec![],
            demo_renderer,
        ),
    ]);

    let cases = [
        (Placement::Center, (2, 1, 4, 2)),
        (Placement::Top, (0, 0, 8, 2)),
        (Placement::Bottom, (0, 2, 8, 2)),
        (Placement::Left, (0, 0, 4, 4)),
        (Placement::Right, (4, 0, 4, 4)),
    ];

    for (placement, (x, y, width, height)) in cases {
        let scene = Scene {
            frame_rate: 24,
            color: false,
            instances: vec![AnimationInstance {
                id: "demo-1".to_string(),
                preset: "demo".to_string(),
                options: BTreeMap::new(),
                placement,
                layer: Layer::Normal,
                z_index: 0,
                enabled: true,
            }],
        };

        let frame = render_scene_frame(&scene, &registry, 1, 0.0, 8, 4).unwrap();
        assert_filled_region(&frame, x, y, width, height);
    }
}

#[test]
fn render_scene_frame_wraps_instance_seed_derivation() {
    RECORDED_SEEDS.lock().unwrap().clear();
    let registry = ascii_animation::presets::PresetRegistry::new(vec![
        ascii_animation::presets::PresetDescriptor::new(
            "demo",
            "Demo",
            "Test preset",
            vec![],
            seed_recording_renderer,
        ),
    ]);
    let scene = Scene {
        frame_rate: 24,
        color: false,
        instances: vec![
            AnimationInstance {
                id: "demo-1".to_string(),
                preset: "demo".to_string(),
                options: BTreeMap::new(),
                placement: Placement::Fill,
                layer: Layer::Normal,
                z_index: 0,
                enabled: true,
            },
            AnimationInstance {
                id: "demo-2".to_string(),
                preset: "demo".to_string(),
                options: BTreeMap::new(),
                placement: Placement::Fill,
                layer: Layer::Normal,
                z_index: 0,
                enabled: true,
            },
        ],
    };

    render_scene_frame(&scene, &registry, u64::MAX, 0.0, 6, 3).unwrap();

    assert_eq!(*RECORDED_SEEDS.lock().unwrap(), vec![u64::MAX, 0]);
}

#[test]
fn direct_scene_renders_non_empty_frame() {
    let args = parse_run([
        "ascii-animation",
        "run",
        "galaxy",
        "--stars",
        "100",
        "--no-color",
    ]);
    let registry = build_default_registry();
    let scene = scene_from_run_args(&args, &registry).unwrap();

    let frame = render_scene_frame(&scene, &registry, 1, 0.0, 40, 16).unwrap();
    let text = render_to_ansi(&frame, false);

    assert_eq!(text.lines().count(), 16);
    assert!(text.chars().any(|ch| ch != ' ' && ch != '\n'));
}

#[test]
fn direct_text_art_scene_renders_non_empty_frame() {
    let args = parse_run([
        "ascii-animation",
        "run",
        "text-art",
        "--text",
        "OK",
        "--text-bg",
        "none",
        "--text-effect",
        "none",
        "--no-color",
    ]);
    let registry = build_default_registry();
    let scene = scene_from_run_args(&args, &registry).unwrap();

    let frame = render_scene_frame(&scene, &registry, 1, 0.0, 40, 16).unwrap();
    let text = render_to_ansi(&frame, false);

    assert_eq!(text.lines().count(), 16);
    assert!(text.chars().any(|ch| ch != ' ' && ch != '\n'));
}

#[test]
fn render_scene_frame_respects_custom_placement() {
    let registry = build_default_registry();
    let mut scene = galaxy_scene(false);
    scene.instances[0].placement = Placement::Custom {
        x: 20,
        y: 0,
        width: 20,
        height: 16,
    };

    let frame = render_scene_frame(&scene, &registry, 1, 0.0, 40, 16).unwrap();

    assert!((0..20).all(|x| (0..16).all(|y| frame.get(x, y).unwrap().ch == ' ')));
    assert!((20..40).any(|x| (0..16).any(|y| frame.get(x, y).unwrap().ch != ' ')));
}

#[test]
fn centered_runtime_viewport_crops_from_logical_scene_center() {
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
            placement: Placement::Custom {
                x: 54,
                y: 22,
                width: 2,
                height: 2,
            },
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        }],
    };

    let frame = render_centered_scene_frame(&scene, &registry, 1, 0.0, 10, 6).unwrap();

    assert_filled_region(&frame, 4, 2, 2, 2);
}

#[test]
fn centered_runtime_viewport_pads_logical_scene_center() {
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
            placement: Placement::Custom {
                x: 54,
                y: 22,
                width: 2,
                height: 2,
            },
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        }],
    };

    let frame = render_centered_scene_frame(&scene, &registry, 1, 0.0, 114, 50).unwrap();

    assert_filled_region(&frame, 56, 24, 2, 2);
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
