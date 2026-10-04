// Perspective/depth model adapted from termflix f2f2a3ab, src/animations/starfield.rs.
// Analytic recycling replaces frame-dependent resets; terminal cells use 2:1 aspect.
// See THIRD_PARTY_NOTICES.
use std::collections::BTreeMap;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::{palette, OptionDescriptor, OptionGroup, OptionValue, PresetDescriptor};
use crate::render::{AnimationRenderer, FrameBuffer, RenderContext, Rgb};
use crate::Result;

pub fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new("starfield", "Depth Starfield", "Fly forward through seeded stars with perspective and short streaks", vec![
        OptionDescriptor::int("starfield-count", "Star Count", 180, 1, 1200, true).with_help("Seeded star population; changing this restarts this Animation instance."),
        OptionDescriptor::float("starfield-speed", "Speed", 0.3, 0.0, 4.0, false).with_group(OptionGroup::Motion).with_help("Depth units per second; stars recycle at the near plane or region edge."),
        OptionDescriptor::int("starfield-streak", "Streak Length", 2, 0, 12, false).with_group(OptionGroup::Style).with_help("Maximum extra cells trailing toward the vanishing point; zero draws points."),
        palette::option("starfield-palette", "ice"),
    ], boxed_renderer)
}

struct Options { count: usize, speed: f64, streak: usize, palette: &'static [Rgb] }
impl Options {
    fn parse(options: &BTreeMap<String, OptionValue>) -> Result<Self> {
        let values = descriptor().validate_options(options)?;
        Ok(Self { count: palette::number(&values, "starfield-count") as usize, speed: palette::number(&values, "starfield-speed"), streak: palette::number(&values, "starfield-streak") as usize, palette: palette::colors(palette::name(&values, "starfield-palette")) })
    }
}
struct Star { x: f64, y: f64, phase: f64, speed_factor: f64 }
struct StarfieldRenderer {
    options: Options,
    stars: Vec<Star>,
    distance: f64,
    last_time: f64,
    pixels: Vec<(char, f64)>,
}

pub fn boxed_renderer(options: &BTreeMap<String, OptionValue>, seed: u64) -> Result<Box<dyn AnimationRenderer>> {
    let options = Options::parse(options)?;
    let mut rng = StdRng::seed_from_u64(seed);
    let stars = (0..options.count).map(|_| Star { x: rng.gen_range(-0.5..0.5), y: rng.gen_range(-0.5..0.5), phase: rng.gen(), speed_factor: rng.gen_range(0.5..1.5) }).collect();
    Ok(Box::new(StarfieldRenderer { options, stars, distance: 0.0, last_time: 0.0, pixels: Vec::new() }))
}

impl AnimationRenderer for StarfieldRenderer {
    fn depends_on_dimensions(&self) -> bool { true }
    fn reconfigure(&mut self, options: &BTreeMap<String, OptionValue>) -> Result<()> {
        self.options = Options::parse(options)?;
        Ok(())
    }
    fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext) {
        if context.width == 0 || context.height == 0 { return; }
        let delta = (context.elapsed_seconds - self.last_time).max(0.0);
        self.distance += delta * self.options.speed;
        self.last_time = context.elapsed_seconds;
        self.pixels.resize(usize::from(context.width) * usize::from(context.height), (' ', -1.0));
        self.pixels.fill((' ', -1.0));
        let cx = (f64::from(context.width) - 1.0) / 2.0;
        let cy = (f64::from(context.height) - 1.0) / 2.0;
        let focal = (f64::from(context.width) / 2.0).min(f64::from(context.height)) * 0.5;
        for star in &self.stars {
            // Recycle before projection can cross the near plane or region edge.
            let near = 0.04_f64.max(star.x.abs() * focal * 2.0 / (cx + 0.5)).max(star.y.abs() * focal / (cy + 0.5));
            let span = 1.0 - near;
            let z = near + (star.phase * span - self.distance * star.speed_factor).rem_euclid(span);
            let px = cx + star.x / z * focal * 2.0;
            let py = cy + star.y / z * focal;
            let brightness = (1.0 - z).clamp(0.0, 1.0);
            let dx = cx - px;
            let dy = cy - py;
            let extent = dx.abs().max(dy.abs()).max(1.0);
            let streak = ((self.options.streak as f64 * brightness).round() as usize)
                .min(extent.floor() as usize);
            for step in 0..=streak {
                let x = (px + dx / extent * step as f64).round() as i32;
                let y = (py + dy / extent * step as f64).round() as i32;
                if x < 0 || y < 0 || x >= i32::from(context.width) || y >= i32::from(context.height) { continue; }
                let intensity = brightness * (1.0 - step as f64 / (streak + 1) as f64);
                let glyph = if step > 0 || intensity < 0.35 { '.' } else if intensity < 0.7 { '+' } else { '*' };
                let pixel = &mut self.pixels[y as usize * usize::from(context.width) + x as usize];
                if intensity > pixel.1 { *pixel = (glyph, intensity); }
            }
        }
        for (index, &(glyph, intensity)) in self.pixels.iter().enumerate() {
            if glyph != ' ' {
                palette::put(frame, context, (index % usize::from(context.width)) as i32, (index / usize::from(context.width)) as i32, glyph, palette::color(self.options.palette, intensity));
            }
        }
    }
}
