// Wave interference adapted from termflix f2f2a3ab, src/animations/plasma.rs.
// See THIRD_PARTY_NOTICES for provenance and MIT permission.
use std::collections::BTreeMap;

use super::{palette, OptionDescriptor, OptionGroup, OptionValue, PresetDescriptor};
use crate::render::{AnimationRenderer, FrameBuffer, RenderContext};
use crate::Result;

pub fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new("plasma", "Plasma Field", "A dense wave field; choose Fill Placement and Background Layer behind text", vec![
        OptionDescriptor::float("plasma-frequency", "Frequency", 8.0, 1.0, 24.0, false).with_group(OptionGroup::Basics).with_help("Spatial wave frequency; lower values produce broader waves."),
        OptionDescriptor::float("plasma-speed", "Speed", 0.8, 0.0, 5.0, false).with_group(OptionGroup::Motion).with_help("Wave phase speed per second; zero freezes motion."),
        OptionDescriptor::float("plasma-contrast", "Contrast", 1.0, 0.1, 3.0, false).with_group(OptionGroup::Style).with_help("Density contrast around the midpoint; this is a dense background field."),
        palette::option("plasma-palette", "rainbow"),
    ], boxed_renderer)
}

struct PlasmaRenderer {
    frequency: f64,
    speed: f64,
    contrast: f64,
    palette: &'static [crate::render::Rgb],
}

pub fn boxed_renderer(options: &BTreeMap<String, OptionValue>, _seed: u64) -> Result<Box<dyn AnimationRenderer>> {
    Ok(Box::new(PlasmaRenderer::parse(options)?))
}

impl PlasmaRenderer {
    fn parse(options: &BTreeMap<String, OptionValue>) -> Result<Self> {
        let values = descriptor().validate_options(options)?;
        Ok(Self {
            frequency: palette::number(&values, "plasma-frequency"),
            speed: palette::number(&values, "plasma-speed"),
            contrast: palette::number(&values, "plasma-contrast"),
            palette: palette::colors(palette::name(&values, "plasma-palette")),
        })
    }
}

impl AnimationRenderer for PlasmaRenderer {
    fn reconfigure(&mut self, options: &BTreeMap<String, OptionValue>) -> Result<()> {
        *self = Self::parse(options)?;
        Ok(())
    }

    fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext) {
        const RAMP: &[u8] = b".:-=+*#%@";
        let t = context.elapsed_seconds * self.speed;
        for y in 0..context.height {
            let fy = f64::from(y) / f64::from(context.height) * self.frequency;
            for x in 0..context.width {
                let fx = f64::from(x) / f64::from(context.width) * self.frequency;
                let v1 = (fx + t).sin();
                let v2 = ((fy * 1.5 + t * 0.7).sin() + (fx * 0.7 + t * 1.3).cos()) * 0.5;
                let v3 = (fx.hypot(fy) * 0.3 - t).sin();
                let v4 = (fx * 0.5 + fy * 0.5 + t * 0.5).sin() * 0.7;
                let intensity = ((v1 + v2 + v3 + v4) * 0.25 * self.contrast + 0.5).clamp(0.0, 1.0);
                let glyph = RAMP[(intensity * (RAMP.len() - 1) as f64).round() as usize] as char;
                palette::put(frame, context, i32::from(x), i32::from(y), glyph, palette::color(self.palette, intensity));
            }
        }
    }
}
