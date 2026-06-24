use std::collections::BTreeMap;

use ascii_animation::presets::{
    galaxy, OptionDescriptor, OptionKind, OptionValue, PresetDescriptor,
};

fn demo_renderer(
    _options: &BTreeMap<String, OptionValue>,
    _seed: u64,
) -> ascii_animation::Result<Box<dyn ascii_animation::render::AnimationRenderer>> {
    unreachable!("validation tests do not render")
}

fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new(
        "demo",
        "Demo",
        "A demo preset",
        vec![
            OptionDescriptor::int_step("count", "Count", 3, 1, 10, 2, true),
            OptionDescriptor::float("glow", "Glow", 0.5, 0.0, 1.0, false),
            OptionDescriptor::choice(
                "palette",
                "Palette",
                "cosmic",
                vec!["cosmic", "mono"],
                false,
            ),
            OptionDescriptor::bool("enabled", "Enabled", true, false),
            OptionDescriptor::text("message", "Message", "HELLO", 12, true),
        ],
        demo_renderer,
    )
}

#[test]
fn validation_fills_defaults() {
    let values = descriptor().validate_options(&BTreeMap::new()).unwrap();

    assert_eq!(values.get("count"), Some(&OptionValue::Int(3)));
    assert_eq!(values.get("glow"), Some(&OptionValue::Float(0.5)));
    assert_eq!(
        values.get("palette"),
        Some(&OptionValue::Choice("cosmic".to_string()))
    );
    assert_eq!(values.get("enabled"), Some(&OptionValue::Bool(true)));
    assert_eq!(
        values.get("message"),
        Some(&OptionValue::Text("HELLO".to_string()))
    );
}

#[test]
fn validation_rejects_unknown_option() {
    let mut raw = BTreeMap::new();
    raw.insert("missing".to_string(), OptionValue::Int(1));

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(err, "unknown option `missing` for preset `demo`");
}

#[test]
fn validation_rejects_integer_step_mismatch() {
    let mut raw = BTreeMap::new();
    raw.insert("count".to_string(), OptionValue::Int(4));

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(
        err,
        "option `count` is out of range: expected 1..=10, got 4"
    );
}

#[test]
fn validation_rejects_out_of_range_float() {
    let mut raw = BTreeMap::new();
    raw.insert("glow".to_string(), OptionValue::Float(1.5));

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(
        err,
        "option `glow` is out of range: expected 0..=1, got 1.5"
    );
}

#[test]
fn validation_rejects_invalid_choice() {
    let mut raw = BTreeMap::new();
    raw.insert(
        "palette".to_string(),
        OptionValue::Choice("bad".to_string()),
    );

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(
        err,
        "invalid choice for `palette`: expected one of [\"cosmic\", \"mono\"], got `bad`"
    );
}

#[test]
fn validation_rejects_text_longer_than_max_len() {
    let mut raw = BTreeMap::new();
    raw.insert(
        "message".to_string(),
        OptionValue::Text("HELLO WORLD!!".to_string()),
    );

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(
        err,
        "option `message` is too long: expected at most 12 characters, got 13"
    );
}

#[test]
fn validation_rejects_non_ascii_text() {
    let mut raw = BTreeMap::new();
    raw.insert(
        "message".to_string(),
        OptionValue::Text("HELLO ☃".to_string()),
    );

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(
        err,
        "invalid value for `message`: expected ASCII text, got HELLO ☃"
    );
}

#[test]
fn descriptors_expose_option_kind_for_tui_and_cli() {
    let preset = descriptor();

    assert_eq!(
        preset.options()[0].kind(),
        &OptionKind::Int {
            min: 1,
            max: 10,
            step: 2,
        }
    );
    assert!(preset.options()[0].rebuilds_state());
}

#[test]
fn galaxy_stars_descriptor_requires_step_50() {
    let descriptor = galaxy::descriptor();

    assert_eq!(
        descriptor
            .options()
            .iter()
            .find(|option| option.name() == "stars")
            .unwrap()
            .kind(),
        &OptionKind::Int {
            min: 100,
            max: 1200,
            step: 50,
        }
    );
}
