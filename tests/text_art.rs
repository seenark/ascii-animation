use std::collections::BTreeMap;
use figlet_rs::FIGlet;

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

fn trim_trailing_blank_lines(content: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn figlet_font_file(name: &str) -> FIGlet {
    let path = format!("{}/figlet/{name}.flf", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(path).unwrap();
    let content = match String::from_utf8(bytes) {
        Ok(content) => content,
        Err(err) => err.into_bytes().into_iter().map(char::from).collect(),
    };
    FIGlet::from_content(&trim_trailing_blank_lines(&content)).unwrap()
}

fn utf8_figlet_font_file(name: &str) -> FIGlet {
    let path = format!("{}/figlet/{name}.flf", env!("CARGO_MANIFEST_DIR"));
    let content = String::from_utf8(std::fs::read(path).unwrap()).unwrap();
    FIGlet::from_content(&trim_trailing_blank_lines(&content)).unwrap()
}

fn clean_text_options(text: &str) -> BTreeMap<String, OptionValue> {
    let mut options = text_art::descriptor().defaults();
    options.insert("text".to_string(), OptionValue::Text(text.to_string()));
    options.insert(
        "text-font".to_string(),
        OptionValue::Choice("Standard".to_string()),
    );
    options.insert(
        "text-bg".to_string(),
        OptionValue::Choice("none".to_string()),
    );
    options.insert(
        "text-effect".to_string(),
        OptionValue::Choice("none".to_string()),
    );
    options.insert("text-amp".to_string(), OptionValue::Float(0.0));
    options.insert(
        "text-drop-shadow".to_string(),
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


fn text_lines(frame: &FrameBuffer, width: u16, height: u16) -> Vec<String> {
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| frame.get(x, y).unwrap().ch)
                .collect::<String>()
                .trim_end()
                .to_string()
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
    let font_option = descriptor
        .options()
        .iter()
        .find(|option| option.name() == "text-font")
        .unwrap();
    let font_choices = match font_option.kind() {
        ascii_animation::presets::OptionKind::Choice { choices } => choices,
        other => panic!("text-font must be choice option, got {other:?}"),
    };

    assert_eq!(descriptor.name(), "text-art");
    assert_eq!(defaults.get("text").unwrap().as_cli_value(), "HELLO");
    assert_eq!(
        defaults.get("text-overflow").unwrap().as_cli_value(),
        "extend"
    );
    assert_eq!(defaults.get("text-font").unwrap().as_cli_value(), "Standard");
    assert!(!defaults.contains_key("text-fill"));
    assert!(!defaults.contains_key("text-scale"));
    assert!(!defaults.contains_key("text-spacing"));
    assert!(!defaults.contains_key("text-block-shadow"));
    assert_eq!(
        defaults.get("text-palette").unwrap().as_cli_value(),
        "cosmic"
    );
    assert_eq!(defaults.get("text-effect").unwrap().as_cli_value(), "none");
    assert_eq!(
        defaults.get("text-color-mode").unwrap().as_cli_value(),
        "gradient-h"
    );
    assert_eq!(defaults.get("text-bg").unwrap().as_cli_value(), "stars");
    assert_eq!(defaults.get("text-speed").unwrap().as_cli_value(), "1.5");
    assert_eq!(defaults.get("text-amp").unwrap().as_cli_value(), "2.5");
    assert_eq!(defaults.get("text-freq").unwrap().as_cli_value(), "1");
    assert_eq!(defaults.get("text-glitch").unwrap().as_cli_value(), "0.15");
    assert_eq!(defaults.get("text-bright").unwrap().as_cli_value(), "1");
    assert_eq!(defaults.get("text-voffset").unwrap().as_cli_value(), "0");
    assert_eq!(
        defaults.get("text-drop-shadow").unwrap().as_cli_value(),
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
    assert!(font_choices.windows(2).all(|w| {
        w[0].to_ascii_lowercase() <= w[1].to_ascii_lowercase()
    }));
    assert!(font_choices.contains(&"ANSI Regular".to_string()));
    assert!(font_choices.contains(&"Block".to_string()));
    assert!(font_choices.contains(&"DOS Rebel".to_string()));
    assert!(font_choices.contains(&"Dot Matrix".to_string()));
    assert!(font_choices.contains(&"Standard".to_string()));
}

#[test]
fn text_art_standard_figlet_renders_known_output() {
    let mut options = clean_text_options("OK");
    options.insert(
        "text-font".to_string(),
        OptionValue::Choice("Standard".to_string()),
    );
    let expected_figlet = figlet_font_file("Standard")
        .convert("OK")
        .unwrap()
        .to_string();
    let expected_rows: Vec<String> = expected_figlet
        .lines()
        .map(str::trim_end)
        .map(ToOwned::to_owned)
        .collect();
    let width = expected_rows
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap() as u16;
    let height = expected_rows.len() as u16;
    let frame = render_text_frame(&options, width, height);

    assert_eq!(text_lines(&frame, width, height), expected_rows);
}

#[test]
fn text_art_ansi_shadow_preserves_utf8_glyphs() {
    let mut options = clean_text_options("A");
    options.insert(
        "text-font".to_string(),
        OptionValue::Choice("ANSI Shadow".to_string()),
    );
    options.insert(
        "text-effect".to_string(),
        OptionValue::Choice("none".to_string()),
    );
    options.insert(
        "text-bg".to_string(),
        OptionValue::Choice("none".to_string()),
    );
    options.insert(
        "text-palette".to_string(),
        OptionValue::Choice("mono".to_string()),
    );
    options.insert(
        "text-color-mode".to_string(),
        OptionValue::Choice("solid".to_string()),
    );
    options.insert("text-glow".to_string(), OptionValue::Bool(false));

    let expected_rows: Vec<String> = utf8_figlet_font_file("ANSI Shadow")
        .convert("A")
        .unwrap()
        .to_string()
        .lines()
        .map(str::trim_end)
        .map(ToOwned::to_owned)
        .collect();
    let width = expected_rows
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap() as u16;
    let height = expected_rows.len() as u16;
    let frame = render_text_frame(&options, width, height);
    let rendered_rows = text_lines(&frame, width, height);

    assert_eq!(rendered_rows, expected_rows);
    assert!(expected_rows.iter().any(|row| row.contains("█████╗")));
    assert!(rendered_rows.iter().all(|row| !row.contains('â')));
}


#[test]
fn text_art_none_effect_keeps_bitmap_static_even_with_motion_options() {
    let mut options = clean_text_options("H");
    options.insert(
        "text-effect".to_string(),
        OptionValue::Choice("none".to_string()),
    );
    options.insert("text-amp".to_string(), OptionValue::Float(8.0));
    options.insert("text-freq".to_string(), OptionValue::Float(4.0));
    options.insert("text-speed".to_string(), OptionValue::Float(5.0));

    let first_frame = render_text_frame_at(&options, 15, 9, 0.0);
    let later_frame = render_text_frame_at(&options, 15, 9, 2.75);

    assert_eq!(
        text_lines(&first_frame, 15, 9),
        text_lines(&later_frame, 15, 9)
    );
}

#[test]
fn text_art_figlet_size_is_not_scaled() {
    let mut options = clean_text_options("A");
    options.insert("text-scale".to_string(), OptionValue::Float(2.0));

    let err = text_art::renderer(&options, 7).unwrap_err().to_string();

    assert_eq!(err, "unknown option `text-scale` for preset `text-art`");
}

#[test]
fn text_art_drop_shadow_does_not_replace_figlet_glyphs() {
    let mut base = clean_text_options("H");
    base.insert(
        "text-font".to_string(),
        OptionValue::Choice("Standard".to_string()),
    );

    let frame_without_shadows = render_text_frame(&base, 15, 9);
    assert_eq!(count_char(&frame_without_shadows, '▒'), 0);

    let main_cells: Vec<(u16, u16)> = (0..9)
        .flat_map(|y| (0..15).map(move |x| (x, y)))
        .filter(|(x, y)| frame_without_shadows.get(*x, *y).unwrap().ch != ' ')
        .collect();
    assert!(!main_cells.is_empty());

    let mut drop_shadow = base.clone();
    drop_shadow.insert("text-drop-shadow".to_string(), OptionValue::Bool(true));
    let frame_with_drop_shadow = render_text_frame(&drop_shadow, 15, 9);
    assert!(count_char(&frame_with_drop_shadow, '▒') > 0);

    for (x, y) in &main_cells {
        assert_ne!(
            frame_without_shadows.get(*x, *y).unwrap().ch,
            '▒',
            "baseline glyph cell at ({x}, {y}) must not already be shadow"
        );
        assert_eq!(
            frame_with_drop_shadow.get(*x, *y).unwrap().ch,
            frame_without_shadows.get(*x, *y).unwrap().ch,
            "drop shadow must not replace main glyph cell at ({x}, {y})"
        );
    }

    let shadow_cells: Vec<(u16, u16)> = (0..9)
        .flat_map(|y| (0..15).map(move |x| (x, y)))
        .filter(|(x, y)| frame_with_drop_shadow.get(*x, *y).unwrap().ch == '▒')
        .collect();
    assert!(!shadow_cells.is_empty());
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

    let mut text_cells: Vec<(u16, Option<Rgb>)> = (0..9)
        .flat_map(|y| (0..15).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let cell = frame.get(x, y).unwrap();
            (cell.ch != ' ').then_some((x, cell.color))
        })
        .collect();
    text_cells.sort_by_key(|(x, _)| *x);

    let left = text_cells.first().unwrap();
    assert_eq!(left.1, Some(Rgb::new(85, 0, 0)));
    assert!(text_cells.iter().any(|(_, color)| *color == Some(Rgb::new(255, 255, 255))));
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
    assert!(
        (0..10)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .any(|(x, y)| {
                let cell = frame.get(x, y).unwrap();
                cell.ch != ' ' && cell.ch != '+'
            })
    );
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
    assert!(frame.cells().iter().any(|cell| cell.ch != ' '));
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
        "invalid choice for `text-effect`: expected one of [\"none\", \"wave\", \"pulse\", \"glitch\", \"scan\", \"rain\", \"fire\", \"matrix\", \"dissolve\", \"bounce\", \"typewriter\", \"strobe\", \"neon-flicker\"], got `spin`"
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

    assert_eq!(text_lines(&first_frame, 10, 9), text_lines(&later_frame, 10, 9));
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
    let later_frame = render_text_frame_at(&options, 10, 9, 6.2);

    assert_ne!(text_lines(&first_frame, 10, 9), text_lines(&later_frame, 10, 9));
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

    assert_eq!(text_lines(&first_frame, 10, 9), text_lines(&later_frame, 10, 9));
}

#[test]
fn default_registry_includes_text_art() {
    let registry = build_default_registry();
    assert!(registry.get("text-art").is_ok());
}
