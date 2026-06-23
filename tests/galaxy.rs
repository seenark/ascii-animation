use ascii_animation::presets::galaxy;
use ascii_animation::render::{AnimationRenderer, FrameBuffer, RenderContext};
use ascii_animation::scene::Layer;

#[test]
fn galaxy_descriptor_has_required_options_and_defaults() {
    let descriptor = galaxy::descriptor();
    let defaults = descriptor.defaults();

    assert_eq!(descriptor.name(), "galaxy");
    assert_eq!(defaults.get("arms").unwrap().as_cli_value(), "3");
    assert_eq!(defaults.get("stars").unwrap().as_cli_value(), "600");
    assert_eq!(defaults.get("speed").unwrap().as_cli_value(), "20");
    assert_eq!(defaults.get("size").unwrap().as_cli_value(), "70");
    assert_eq!(defaults.get("twist").unwrap().as_cli_value(), "0.45");
    assert_eq!(defaults.get("noise").unwrap().as_cli_value(), "0.15");
    assert_eq!(defaults.get("glow").unwrap().as_cli_value(), "0.45");
    assert_eq!(defaults.get("twinkle").unwrap().as_cli_value(), "0.35");
    assert_eq!(defaults.get("palette").unwrap().as_cli_value(), "cosmic");
    assert_eq!(defaults.get("gradient").unwrap().as_cli_value(), "smooth");
}

#[test]
fn galaxy_frame_is_deterministic_with_fixed_seed() {
    let descriptor = galaxy::descriptor();
    let options = descriptor.defaults();
    let mut renderer = galaxy::renderer(&options, 7).unwrap();
    let mut frame = FrameBuffer::new(24, 12);

    renderer.render(
        &mut frame,
        RenderContext {
            elapsed_seconds: 0.25,
            layer: Layer::Normal,
            z_index: 0,
            order: 0,
            x_offset: 0,
            y_offset: 0,
            width: 24,
            height: 12,
        },
    );

    insta::assert_snapshot!(frame.to_plain_text());
}

#[test]
fn galaxy_renderer_rejects_unknown_palette_choice() {
    let mut options = galaxy::descriptor().defaults();
    options.insert(
        "palette".to_string(),
        ascii_animation::presets::OptionValue::Choice("aurora".to_string()),
    );

    let err = galaxy::renderer(&options, 7).unwrap_err().to_string();

    assert_eq!(
        err,
        "invalid choice for `palette`: expected one of [\"cosmic\", \"stardust\", \"nebula\", \"rainbow\", \"ice\", \"mono\"], got `aurora`"
    );
}

#[test]
fn galaxy_renderer_rejects_unknown_gradient_choice() {
    let mut options = galaxy::descriptor().defaults();
    options.insert(
        "gradient".to_string(),
        ascii_animation::presets::OptionValue::Choice("plasma".to_string()),
    );

    let err = galaxy::renderer(&options, 7).unwrap_err().to_string();

    assert_eq!(
        err,
        "invalid choice for `gradient`: expected one of [\"smooth\", \"classic\", \"starry\", \"block\"], got `plasma`"
    );
}

#[test]
fn galaxy_renderer_rejects_arms_below_minimum() {
    let mut options = galaxy::descriptor().defaults();
    options.insert("arms".to_string(), ascii_animation::presets::OptionValue::Int(0));

    let err = galaxy::renderer(&options, 7).unwrap_err().to_string();

    assert_eq!(
        err,
        "option `arms` is out of range: expected 1..=10, got 0"
    );
}

#[test]
fn galaxy_renderer_rejects_stars_below_minimum() {
    let mut options = galaxy::descriptor().defaults();
    options.insert("stars".to_string(), ascii_animation::presets::OptionValue::Int(-1));

    let err = galaxy::renderer(&options, 7).unwrap_err().to_string();

    assert_eq!(
        err,
        "option `stars` is out of range: expected 100..=1200, got -1"
    );
}
