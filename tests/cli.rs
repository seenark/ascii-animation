use clap::Parser;

use ascii_animation::cli::{scene_from_run_args, Cli, Command};
use ascii_animation::presets::build_default_registry;

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
fn rejects_invalid_galaxy_option_range() {
    let cli = Cli::parse_from(["ascii-animation", "run", "galaxy", "--arms", "99"]);
    let Command::Run(args) = cli.command else { panic!("expected run command") };

    let err = scene_from_run_args(&args, &build_default_registry()).unwrap_err().to_string();

    assert_eq!(err, "option `arms` is out of range: expected 1..=10, got 99");
}
