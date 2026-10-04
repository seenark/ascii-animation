// Stream/trail model adapted from termflix f2f2a3ab, src/animations/matrix.rs.
// ASCII glyphs and accepted-time motion replace upstream per-draw randomness.
// See THIRD_PARTY_NOTICES.
use std::collections::BTreeMap;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::{palette, OptionDescriptor, OptionGroup, OptionValue, PresetDescriptor};
use crate::render::{AnimationRenderer, FrameBuffer, RenderContext, Rgb};
use crate::Result;

pub fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new("matrix", "Digital Rain", "Seeded ASCII streams with bright heads and fading transparent trails", vec![
        OptionDescriptor::float("matrix-density", "Density", 0.4, 0.0, 1.0, true).with_help("Fraction of columns with a stream; changing this restarts this Animation instance."),
        OptionDescriptor::float("matrix-speed", "Speed", 10.0, 0.0, 40.0, false).with_group(OptionGroup::Motion).with_help("Average rows per second; zero freezes stream motion."),
        OptionDescriptor::int("matrix-trail", "Trail Length", 8, 1, 40, false).with_group(OptionGroup::Style).with_help("Maximum trail length in rows; untouched cells stay transparent."),
        palette::option("matrix-palette", "green"),
    ], boxed_renderer)
}

struct Options {
    density: f64,
    speed: f64,
    trail: usize,
    palette: &'static [Rgb],
}

impl Options {
    fn parse(options: &BTreeMap<String, OptionValue>) -> Result<Self> {
        let values = descriptor().validate_options(options)?;
        Ok(Self {
            density: palette::number(&values, "matrix-density"),
            speed: palette::number(&values, "matrix-speed"),
            trail: palette::number(&values, "matrix-trail") as usize,
            palette: palette::colors(palette::name(&values, "matrix-palette")),
        })
    }
}

struct Stream { x: u16, head: f64, speed_factor: f64, glyph_seed: u64 }
struct MatrixRenderer {
    options: Options,
    seed: u64,
    streams: Vec<Stream>,
    dimensions: Option<(u16, u16)>,
    last_time: f64,
}

pub fn boxed_renderer(options: &BTreeMap<String, OptionValue>, seed: u64) -> Result<Box<dyn AnimationRenderer>> {
    Ok(Box::new(MatrixRenderer { options: Options::parse(options)?, seed, streams: Vec::new(), dimensions: None, last_time: 0.0 }))
}

impl AnimationRenderer for MatrixRenderer {
    fn depends_on_dimensions(&self) -> bool { true }

    fn reconfigure(&mut self, options: &BTreeMap<String, OptionValue>) -> Result<()> {
        self.options = Options::parse(options)?;
        Ok(())
    }

    fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext) {
        if context.width == 0 || context.height == 0 { return; }
        if self.dimensions != Some((context.width, context.height)) {
            let mut rng = StdRng::seed_from_u64(self.seed);
            self.streams.clear();
            for x in 0..context.width {
                if rng.gen::<f64>() < self.options.density {
                    self.streams.push(Stream { x, head: rng.gen_range(0.0..f64::from(context.height)), speed_factor: rng.gen_range(0.6..1.4), glyph_seed: rng.gen() });
                }
            }
            self.dimensions = Some((context.width, context.height));
        }
        let delta = (context.elapsed_seconds - self.last_time).max(0.0);
        self.last_time = context.elapsed_seconds;
        let cycle = f64::from(context.height) + self.options.trail as f64;
        for stream in &mut self.streams {
            if delta > 0.0 && self.options.speed > 0.0 {
                stream.head = (stream.head + delta * self.options.speed * stream.speed_factor).rem_euclid(cycle);
            }
            let head = stream.head.floor() as i32;
            for age in 0..self.options.trail {
                let y = head - age as i32;
                let brightness = 1.0 - age as f64 / self.options.trail as f64;
                let glyph = if age == 0 { '@' } else if brightness < 0.35 { '.' } else {
                    const GLYPHS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
                    let tick = (stream.head * 3.0).floor() as u64;
                    let hash = stream.glyph_seed.wrapping_add(tick.wrapping_mul(0x9e3779b97f4a7c15)).wrapping_add((age as u64).wrapping_mul(0xbf58476d1ce4e5b9));
                    GLYPHS[((hash ^ (hash >> 32)) % GLYPHS.len() as u64) as usize] as char
                };
                palette::put(frame, context, i32::from(stream.x), y, glyph, palette::color(self.options.palette, brightness));
            }
        }
    }
}
