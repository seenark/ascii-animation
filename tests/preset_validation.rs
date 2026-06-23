use std::collections::BTreeMap;

use ascii_animation::presets::{OptionDescriptor, OptionKind, OptionValue, PresetDescriptor};

fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new(
        "demo",
        "Demo",
        "A demo preset",
        vec![
            OptionDescriptor::int("count", "Count", 3, 1, 10, true),
            OptionDescriptor::float("glow", "Glow", 0.5, 0.0, 1.0, false),
            OptionDescriptor::choice(
                "palette",
                "Palette",
                "cosmic",
                vec!["cosmic", "mono"],
                false,
            ),
            OptionDescriptor::bool("enabled", "Enabled", true, false),
        ],
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
}

#[test]
fn validation_rejects_unknown_option() {
    let mut raw = BTreeMap::new();
    raw.insert("missing".to_string(), OptionValue::Int(1));

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(err, "unknown option `missing` for preset `demo`");
}

#[test]
fn validation_rejects_out_of_range_integer() {
    let mut raw = BTreeMap::new();
    raw.insert("count".to_string(), OptionValue::Int(11));

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(err, "option `count` is out of range: expected 1..=10, got 11");
}

#[test]
fn validation_rejects_out_of_range_float() {
    let mut raw = BTreeMap::new();
    raw.insert("glow".to_string(), OptionValue::Float(1.5));

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(err, "option `glow` is out of range: expected 0..=1, got 1.5");
}

#[test]
fn validation_rejects_invalid_choice() {
    let mut raw = BTreeMap::new();
    raw.insert("palette".to_string(), OptionValue::Choice("bad".to_string()));

    let err = descriptor().validate_options(&raw).unwrap_err().to_string();

    assert_eq!(
        err,
        "invalid choice for `palette`: expected one of [\"cosmic\", \"mono\"], got `bad`"
    );
}

#[test]
fn descriptors_expose_option_kind_for_tui_and_cli() {
    let preset = descriptor();

    assert_eq!(preset.options()[0].kind(), &OptionKind::Int { min: 1, max: 10 });
    assert!(preset.options()[0].rebuilds_state());
}
