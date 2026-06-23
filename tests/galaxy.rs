use ascii_animation::presets::galaxy;
use ascii_animation::render::buffer::FrameBuffer;
use ascii_animation::render::{AnimationRenderer, RenderContext};
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
