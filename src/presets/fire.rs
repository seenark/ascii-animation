// Heat recurrence adapted from termflix f2f2a3ab, src/animations/fire.rs.
// Top-to-bottom reads preserve previous-step heat; propagation runs at 60 Hz.
// See THIRD_PARTY_NOTICES.
use std::collections::BTreeMap;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::{palette, OptionDescriptor, OptionGroup, OptionValue, PresetDescriptor};
use crate::render::{AnimationRenderer, FrameBuffer, RenderContext, Rgb};
use crate::Result;

pub fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new("fire", "Coherent Fire", "Persistent heat rises, drifts, and cools; cold cells stay transparent", vec![
        OptionDescriptor::float("fire-fuel", "Fuel", 1.0, 0.0, 2.0, false).with_help("Base fuel intensity; zero stops injection while existing heat cools."),
        OptionDescriptor::float("fire-cooling", "Cooling", 0.06, 0.0, 1.0, false).with_group(OptionGroup::Motion).with_help("Maximum heat lost per upward propagation step; larger values shorten flames."),
        OptionDescriptor::float("fire-wind", "Wind", 0.0, -3.0, 3.0, false).with_group(OptionGroup::Motion).with_help("Horizontal drift in cells per step; positive values move flames right."),
        palette::option("fire-palette", "fire"),
    ], boxed_renderer)
}

struct Options { fuel: f64, cooling: f64, wind: f64, palette: &'static [Rgb] }
impl Options {
    fn parse(options: &BTreeMap<String, OptionValue>) -> Result<Self> {
        let values = descriptor().validate_options(options)?;
        Ok(Self { fuel: palette::number(&values, "fire-fuel"), cooling: palette::number(&values, "fire-cooling"), wind: palette::number(&values, "fire-wind"), palette: palette::colors(palette::name(&values, "fire-palette")) })
    }
}
struct FireRenderer {
    options: Options,
    rng: StdRng,
    heat: Vec<f64>,
    dimensions: Option<(u16, u16)>,
    steps: u64,
}

pub fn boxed_renderer(options: &BTreeMap<String, OptionValue>, seed: u64) -> Result<Box<dyn AnimationRenderer>> {
    Ok(Box::new(FireRenderer { options: Options::parse(options)?, rng: StdRng::seed_from_u64(seed), heat: Vec::new(), dimensions: None, steps: 0 }))
}

impl FireRenderer {
    fn inject(&mut self, width: usize, height: usize) {
        let fuel = self.options.fuel.min(1.0);
        for x in 0..width {
            self.heat[(height - 1) * width + x] = fuel * self.rng.gen_range(0.9..=1.0);
        }
    }

    fn step(&mut self, width: usize, height: usize) {
        // Sources in y+1 remain untouched until their destination row is visited.
        // Thus each step propagates exactly one row, not a full-height cascade.
        for y in 0..height.saturating_sub(1) {
            for x in 0..width {
                let drift = self.rng.gen_range(-1..=1) as f64 + self.options.wind;
                let source = (x as f64 - drift).clamp(0.0, (width - 1) as f64);
                let left = source.floor() as usize;
                let right = (left + 1).min(width - 1);
                let fraction = source.fract();
                let decay = self.rng.gen::<f64>() * self.options.cooling / self.options.fuel.max(0.1);
                let heat = self.heat[(y + 1) * width + left] * (1.0 - fraction)
                    + self.heat[(y + 1) * width + right] * fraction;
                self.heat[y * width + x] = (heat - decay).max(0.0);
            }
        }
        self.inject(width, height);
    }
}

impl AnimationRenderer for FireRenderer {
    fn depends_on_dimensions(&self) -> bool { true }
    fn reconfigure(&mut self, options: &BTreeMap<String, OptionValue>) -> Result<()> {
        self.options = Options::parse(options)?;
        Ok(())
    }
    fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext) {
        if context.width == 0 || context.height == 0 { return; }
        let width = usize::from(context.width);
        let height = usize::from(context.height);
        if self.dimensions != Some((context.width, context.height)) {
            self.heat.resize(width * height, 0.0);
            self.heat.fill(0.0);
            self.inject(width, height);
            self.dimensions = Some((context.width, context.height));
        }
        let target_steps = (context.elapsed_seconds * 60.0 + 1e-8).floor() as u64;
        while self.steps < target_steps {
            self.step(width, height);
            self.steps += 1;
        }
        const RAMP: &[u8] = b" .:-=+*#%@";
        for (index, &heat) in self.heat.iter().enumerate() {
            let glyph = RAMP[(heat.clamp(0.0, 1.0) * (RAMP.len() - 1) as f64).floor() as usize] as char;
            if glyph != ' ' {
                palette::put(frame, context, (index % width) as i32, (index / width) as i32, glyph, palette::color(self.options.palette, heat));
            }
        }
    }
}
