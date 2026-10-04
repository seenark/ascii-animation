use std::collections::BTreeMap;

use super::{OptionDescriptor, OptionGroup, OptionValue};
use crate::render::{Cell, FrameBuffer, RenderContext, Rgb};

pub(super) fn option(name: &str, default: &str) -> OptionDescriptor {
    OptionDescriptor::choice(name, "Palette", default, vec!["green", "fire", "cosmic", "ice", "rainbow", "mono"], false)
        .with_group(OptionGroup::Style)
        .with_help("Color ramp; all particle shapes remain readable with --no-color.")
}

pub(super) fn number(values: &BTreeMap<String, OptionValue>, name: &str) -> f64 {
    match &values[name] {
        OptionValue::Float(value) => *value,
        OptionValue::Int(value) => *value as f64,
        _ => unreachable!("descriptor validates numeric options"),
    }
}

pub(super) fn name<'a>(values: &'a BTreeMap<String, OptionValue>, key: &str) -> &'a str {
    match &values[key] {
        OptionValue::Choice(value) => value,
        _ => unreachable!("descriptor validates choice options"),
    }
}

pub(super) fn colors(name: &str) -> &'static [Rgb] {
    const GREEN: [Rgb; 5] = [Rgb::new(0, 35, 0), Rgb::new(0, 90, 0), Rgb::new(0, 160, 0), Rgb::new(30, 230, 50), Rgb::new(220, 255, 220)];
    const FIRE: [Rgb; 5] = [Rgb::new(45, 0, 0), Rgb::new(150, 0, 0), Rgb::new(255, 60, 0), Rgb::new(255, 180, 0), Rgb::new(255, 255, 210)];
    match name {
        "green" => &GREEN,
        "fire" => &FIRE,
        other => super::galaxy::palette(other),
    }
}

pub(super) fn color(palette: &[Rgb], intensity: f64) -> Rgb {
    palette[(intensity.clamp(0.0, 1.0) * (palette.len() - 1) as f64).round() as usize]
}

pub(super) fn put(frame: &mut FrameBuffer, context: RenderContext, x: i32, y: i32, ch: char, color: Rgb) {
    if x < 0 || y < 0 || x >= i32::from(context.width) || y >= i32::from(context.height) {
        return;
    }
    if let (Some(x), Some(y)) = (context.x_offset.checked_add(x as u16), context.y_offset.checked_add(y as u16)) {
        frame.put_cell(x, y, Cell::visible(ch, Some(color), context.layer, context.z_index, context.order));
    }
}
