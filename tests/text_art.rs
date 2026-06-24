use std::collections::BTreeMap;

use ascii_animation::presets::{build_default_registry, text_art, OptionValue};
use ascii_animation::render::{AnimationRenderer, FrameBuffer, RenderContext, Rgb};
use ascii_animation::scene::Layer;

fn render_context(width: u16, height: u16, elapsed_seconds: f64) -> RenderContext {
    RenderContext {
        elapsed_seconds,
        layer: Layer::Normal,
        z_index: 0,
        order: 0,
        x_offset: 0,
        y_offset: 0,
        width,
        height,
    }
}

fn clean_text_options(text: &str) -> BTreeMap<String, OptionValue> {
    let mut options = text_art::descriptor().defaults();
    options.insert("text".to_string(), OptionValue::Text(text.to_string()));
    options.insert(
        "text-fill".to_string(),
        OptionValue::Choice("hash".to_string()),
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
    options.insert("text-scale".to_string(), OptionValue::Float(1.0));
    options.insert("text-spacing".to_string(), OptionValue::Int(2));
    options.insert(
        "text-drop-shadow".to_string(),
        OptionValue::Bool(false),
    );
    options.insert(
        "text-block-shadow".to_string(),
        OptionValue::Bool(false),
    );
    options.insert("text-glow".to_string(), OptionValue::Bool(false));
    options.insert("text-border".to_string(), OptionValue::Bool(false));
    options.insert("text-reflection".to_string(), OptionValue::Bool(false));
    options.insert("text-particles".to_string(), OptionValue::Bool(false));
    options.insert("text-mirror".to_string(), OptionValue::Bool(false));
    options.insert(
        "text-color-mode".to_string(),
        OptionValue::Choice("solid".to_string()),
    );
    options.insert(
        "text-palette".to_string(),
        OptionValue::Choice("mono".to_string()),
    );
    options
}

fn render_text_frame_at(
    options: &BTreeMap<String, OptionValue>,
    width: u16,
    height: u16,
    elapsed_seconds: f64,
) -> FrameBuffer {
    let mut renderer = text_art::renderer(options, 7).unwrap();
    let mut frame = FrameBuffer::new(width, height);
    renderer.render(&mut frame, render_context(width, height, elapsed_seconds));
    frame
}

fn render_text_frame(options: &BTreeMap<String, OptionValue>, width: u16, height: u16) -> FrameBuffer {
    render_text_frame_at(options, width, height, 0.0)
}

fn visible_window(frame: &FrameBuffer, start_x: u16, start_y: u16, width: u16, height: u16) -> Vec<String> {
    (start_y..start_y + height)
        .map(|y| {
            (start_x..start_x + width)
                .map(|x| if frame.get(x, y).unwrap().ch == ' ' { '.' } else { '#' })
                .collect()
        })
        .collect()
}

fn count_char(frame: &FrameBuffer, target: char) -> usize {
    frame.cells().iter().filter(|cell| cell.ch == target).count()
}

#[test]
fn text_art_descriptor_has_required_options_and_defaults() {
    let descriptor = text_art::descriptor();
    let defaults = descriptor.defaults();

    assert_eq!(descriptor.name(), "text-art");
    assert_eq!(defaults.get("text").unwrap().as_cli_value(), "HELLO");
    assert_eq!(
        defaults.get("text-overflow").unwrap().as_cli_value(),
        "extend"
    );
    assert_eq!(defaults.get("text-font").unwrap().as_cli_value(), "block");
    assert_eq!(defaults.get("text-fill").unwrap().as_cli_value(), "auto");
    assert_eq!(
        defaults.get("text-palette").unwrap().as_cli_value(),
        "cosmic"
    );
    assert_eq!(defaults.get("text-effect").unwrap().as_cli_value(), "wave");
    assert_eq!(
        defaults.get("text-color-mode").unwrap().as_cli_value(),
        "gradient-h"
    );
    assert_eq!(defaults.get("text-bg").unwrap().as_cli_value(), "stars");
    assert_eq!(defaults.get("text-speed").unwrap().as_cli_value(), "1.5");
    assert_eq!(defaults.get("text-scale").unwrap().as_cli_value(), "1");
    assert_eq!(defaults.get("text-amp").unwrap().as_cli_value(), "2.5");
    assert_eq!(defaults.get("text-freq").unwrap().as_cli_value(), "1");
    assert_eq!(defaults.get("text-glitch").unwrap().as_cli_value(), "0.15");
    assert_eq!(defaults.get("text-bright").unwrap().as_cli_value(), "1");
    assert_eq!(defaults.get("text-spacing").unwrap().as_cli_value(), "2");
    assert_eq!(defaults.get("text-voffset").unwrap().as_cli_value(), "0");
    assert_eq!(
        defaults.get("text-drop-shadow").unwrap().as_cli_value(),
        "false"
    );
    assert_eq!(
        defaults.get("text-block-shadow").unwrap().as_cli_value(),
        "false"
    );
    assert_eq!(defaults.get("text-border").unwrap().as_cli_value(), "false");
    assert_eq!(defaults.get("text-glow").unwrap().as_cli_value(), "true");
    assert_eq!(
        defaults.get("text-reflection").unwrap().as_cli_value(),
        "false"
    );
    assert_eq!(
        defaults.get("text-particles").unwrap().as_cli_value(),
        "false"
    );
    assert_eq!(defaults.get("text-mirror").unwrap().as_cli_value(), "false");
}

#[test]
fn text_art_uses_reference_html_glyphs_for_c_g_i() {
    let cases = [
        (
            "C",
            [
                ".###.",
                "#...#",
                "#....",
                "#....",
                "#....",
                "#...#",
                ".###.",
            ],
        ),
        (
            "G",
            [
                ".###.",
                "#...#",
                "#....",
                "#.###",
                "#...#",
                "#...#",
                ".####",
            ],
        ),
        (
            "I",
            [
                ".###.",
                "..#..",
                "..#..",
                "..#..",
                "..#..",
                "..#..",
                ".###.",
            ],
        ),
    ];

    for (text, expected) in cases {
        let options = clean_text_options(text);
        let frame = render_text_frame(&options, 15, 9);
        assert_eq!(visible_window(&frame, 5, 1, 5, 7), expected);
    }
}

#[test]
fn text_art_renderer_draws_scaled_bitmap_text() {
    let mut options = clean_text_options("I");
    options.insert("text-scale".to_string(), OptionValue::Float(2.0));
    let frame = render_text_frame(&options, 20, 16);

    assert_eq!(frame.get(8, 2).unwrap().ch, '#');
    assert_eq!(frame.get(11, 2).unwrap().ch, '#');
    assert_eq!(frame.get(9, 7).unwrap().ch, '#');
    assert_eq!(frame.get(0, 0).unwrap().ch, ' ');
    assert_eq!(frame.get(19, 15).unwrap().ch, ' ');
}

#[test]
fn text_art_block_shadow_is_separate_from_drop_shadow() {
    let mut base = clean_text_options("H");
    base.insert(
        "text-font".to_string(),
        OptionValue::Choice("block".to_string()),
    );

    let frame_without_shadows = render_text_frame(&base, 15, 9);
    assert_eq!(count_char(&frame_without_shadows, '▒'), 0);

    let mut drop_shadow = base.clone();
    drop_shadow.insert("text-drop-shadow".to_string(), OptionValue::Bool(true));
    let frame_with_drop_shadow = render_text_frame(&drop_shadow, 15, 9);
    assert!(count_char(&frame_with_drop_shadow, '▒') > 0);
    assert_eq!(frame_with_drop_shadow.get(5, 1).unwrap().ch, '#');
    assert_eq!(frame_with_drop_shadow.get(9, 1).unwrap().ch, '#');
    assert_eq!(frame_with_drop_shadow.get(10, 2).unwrap().ch, '▒');

    let mut block_shadow = base;
    block_shadow.insert("text-block-shadow".to_string(), OptionValue::Bool(true));
    let frame_with_block_shadow = render_text_frame(&block_shadow, 15, 9);
    assert!(count_char(&frame_with_block_shadow, '▒') > 0);
    assert_eq!(frame_with_block_shadow.get(6, 1).unwrap().ch, '▒');
    assert_ne!(frame_with_block_shadow.get(10, 2).unwrap().ch, '▒');
}

#[test]
fn text_art_palette_uses_reference_seven_stop_fire_gradient() {
    let mut options = clean_text_options("I");
    options.insert(
        "text-palette".to_string(),
        OptionValue::Choice("fire".to_string()),
    );
    options.insert(
        "text-color-mode".to_string(),
        OptionValue::Choice("gradient-h".to_string()),
    );
    let frame = render_text_frame(&options, 15, 9);

    assert_eq!(frame.get(6, 1).unwrap().color, Some(Rgb::new(85, 0, 0)));
    assert_eq!(frame.get(8, 1).unwrap().color, Some(Rgb::new(255, 255, 255)));
}

#[test]
fn text_art_grid_background_draws_behind_text() {
    let mut options = clean_text_options("I");
    options.insert(
        "text-bg".to_string(),
        OptionValue::Choice("grid".to_string()),
    );
    let frame = render_text_frame(&options, 16, 10);

    assert_eq!(frame.get(0, 0).unwrap().ch, '+');
    assert_eq!(frame.get(0, 0).unwrap().color, Some(Rgb::new(26, 34, 64)));
    assert_eq!(frame.get(7, 1).unwrap().ch, '#');
}

#[test]
fn text_art_default_clean_hello_has_no_shadow_glyphs_when_background_disabled() {
    let mut options = text_art::descriptor().defaults();
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
    options.insert("text-border".to_string(), OptionValue::Bool(false));
    options.insert("text-reflection".to_string(), OptionValue::Bool(false));
    options.insert("text-particles".to_string(), OptionValue::Bool(false));
    options.insert("text-mirror".to_string(), OptionValue::Bool(false));

    let frame = render_text_frame(&options, 50, 12);

    assert_eq!(count_char(&frame, '▒'), 0);
    assert!(count_char(&frame, '█') > 0);
}

#[test]
fn text_art_renderer_rejects_invalid_effect_choice() {
    let mut options = text_art::descriptor().defaults();
    options.insert(
        "text-effect".to_string(),
        OptionValue::Choice("spin".to_string()),
    );

    let err = text_art::renderer(&options, 7).unwrap_err().to_string();

    assert_eq!(
        err,
        "invalid choice for `text-effect`: expected one of [\"wave\", \"pulse\", \"glitch\", \"scan\", \"rain\", \"fire\", \"matrix\", \"dissolve\", \"bounce\", \"typewriter\", \"strobe\", \"neon-flicker\"], got `spin`"
    );
}

#[test]
fn text_art_renderer_accepts_sixty_four_chars_and_rejects_sixty_five() {
    let mut accepted = clean_text_options(&"A".repeat(64));
    accepted.insert("text-speed".to_string(), OptionValue::Float(1.0));
    text_art::renderer(&accepted, 7).unwrap();

    let rejected = clean_text_options(&"A".repeat(65));
    let err = text_art::renderer(&rejected, 7).unwrap_err().to_string();

    assert_eq!(
        err,
        "option `text` is too long: expected at most 64 characters, got 65"
    );
}

#[test]
fn text_art_default_overflow_extends_without_horizontal_slide() {
    let mut options = clean_text_options("A     Z");
    options.insert("text-speed".to_string(), OptionValue::Float(1.0));

    let first_frame = render_text_frame_at(&options, 10, 9, 0.0);
    let later_frame = render_text_frame_at(&options, 10, 9, 6.2);

    let expected_a = [
        ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
    ];
    assert_eq!(visible_window(&first_frame, 0, 1, 5, 7), expected_a);
    assert_eq!(visible_window(&later_frame, 0, 1, 5, 7), expected_a);
}

#[test]
fn text_art_slide_overflow_reveals_trailing_characters() {
    let mut options = clean_text_options("A     Z");
    options.insert("text-speed".to_string(), OptionValue::Float(1.0));
    options.insert(
        "text-overflow".to_string(),
        OptionValue::Choice("slide".to_string()),
    );

    let first_frame = render_text_frame_at(&options, 10, 9, 0.0);
    assert_eq!(
        visible_window(&first_frame, 0, 1, 5, 7),
        [
            ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ]
    );

    let trailing_frame = render_text_frame_at(&options, 10, 9, 6.2);
    assert_eq!(
        visible_window(&trailing_frame, 5, 1, 5, 7),
        [
            "#####", "....#", "...#.", "..#..", ".#...", "#....", "#####",
        ]
    );
}
 
#[test]
fn text_art_rejects_invalid_overflow_choice() {
    let mut options = text_art::descriptor().defaults();
    options.insert(
        "text-overflow".to_string(),
        OptionValue::Choice("wrap".to_string()),
    );

    let err = text_art::renderer(&options, 7).unwrap_err().to_string();

    assert_eq!(
        err,
        "invalid choice for `text-overflow`: expected one of [\"extend\", \"slide\"], got `wrap`"
    );
}

#[test]
fn text_art_renderer_defaults_missing_overflow_to_extend_for_saved_scenes() {
    let mut options = clean_text_options("A     Z");
    options.remove("text-overflow");
    options.insert("text-speed".to_string(), OptionValue::Float(1.0));

    let first_frame = render_text_frame_at(&options, 10, 9, 0.0);
    let later_frame = render_text_frame_at(&options, 10, 9, 6.2);

    let expected_a = [
        ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
    ];
    assert_eq!(visible_window(&first_frame, 0, 1, 5, 7), expected_a);
    assert_eq!(visible_window(&later_frame, 0, 1, 5, 7), expected_a);
}

#[test]
fn default_registry_includes_text_art() {
    let registry = build_default_registry();
    assert!(registry.get("text-art").is_ok());
}
