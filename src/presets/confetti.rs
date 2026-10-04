// Burst/closed-form drag, gravity, wobble and fading adapted from TUIOS
// e57ecf3741f8884dd6d033b70188160965bd1a3d, internal/app/celebrate.go.
// No upstream event loop or Unicode glyphs are retained. See THIRD_PARTY_NOTICES.
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::{palette, OptionDescriptor, OptionGroup, OptionValue, PresetDescriptor};
use crate::render::{AnimationRenderer, FrameBuffer, RenderContext, Rgb};
use crate::Result;

const MAX_PARTICLES: usize = 400;
const DRAG: f64 = 1.9;
const GRAVITY: f64 = 30.0;

pub fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new("confetti", "Recurring Confetti", "Bounded ASCII bursts float under drag and gravity, then fade and repeat", vec![
        OptionDescriptor::int("confetti-count", "Burst Count", 64, 1, 400, false).with_help("Particles in each future burst; the total live population never exceeds 400."),
        OptionDescriptor::float("confetti-repeat", "Repeat Interval", 2.0, 0.1, 10.0, false).with_group(OptionGroup::Motion).with_help("Seconds between bursts; particles live for 1–1.5 seconds."),
        palette::option("confetti-palette", "rainbow"),
    ], boxed_renderer)
}

struct Options { count: usize, repeat: f64, palette: &'static [Rgb] }
impl Options {
    fn parse(options: &BTreeMap<String, OptionValue>) -> Result<Self> {
        let values = descriptor().validate_options(options)?;
        Ok(Self { count: palette::number(&values, "confetti-count") as usize, repeat: palette::number(&values, "confetti-repeat"), palette: palette::colors(palette::name(&values, "confetti-palette")) })
    }
}
struct Particle {
    born: f64,
    life: f64,
    vx: f64,
    vy: f64,
    wobble: f64,
    frequency: f64,
    phase: f64,
    kind: usize,
    ink: f64,
}
#[derive(Clone, Copy)]
struct Pixel { glyph: char, color: Rgb, priority: f64 }
struct ConfettiRenderer {
    options: Options,
    rng: StdRng,
    particles: Vec<Particle>,
    next_burst: f64,
    last_time: f64,
    pixels: Vec<Option<Pixel>>,
}

pub fn boxed_renderer(options: &BTreeMap<String, OptionValue>, seed: u64) -> Result<Box<dyn AnimationRenderer>> {
    Ok(Box::new(ConfettiRenderer { options: Options::parse(options)?, rng: StdRng::seed_from_u64(seed), particles: Vec::with_capacity(MAX_PARTICLES), next_burst: 0.0, last_time: 0.0, pixels: Vec::new() }))
}

impl ConfettiRenderer {
    fn burst(&mut self, born: f64, scale: f64) {
        self.particles.retain(|particle| particle.born + particle.life > born);
        let excess = (self.particles.len() + self.options.count).saturating_sub(MAX_PARTICLES);
        if excess > 0 { self.particles.drain(..excess); }
        for index in 0..self.options.count {
            let spread = if index % 6 == 0 { 110.0 } else { 75.0 };
            let theta = (self.rng.gen::<f64>() * 2.0 - 1.0) * spread * PI / 180.0;
            let speed = (0.35 + 0.65 * self.rng.gen::<f64>().sqrt()) * 28.0 * scale;
            self.particles.push(Particle {
                born: born + self.rng.gen_range(0.0..0.07),
                life: self.rng.gen_range(1.0..1.5),
                vx: theta.sin() * speed * 2.0,
                vy: -theta.cos() * speed,
                wobble: self.rng.gen_range(0.4..1.5),
                frequency: self.rng.gen_range(5.0..10.0),
                phase: self.rng.gen_range(0.0..TAU),
                kind: self.rng.gen_range(0..4),
                ink: self.rng.gen(),
            });
        }
    }
}

impl AnimationRenderer for ConfettiRenderer {
    fn depends_on_dimensions(&self) -> bool { true }
    fn reconfigure(&mut self, options: &BTreeMap<String, OptionValue>) -> Result<()> {
        let options = Options::parse(options)?;
        if options.repeat != self.options.repeat && self.next_burst > 0.0 {
            self.next_burst = self.last_time + options.repeat;
        }
        self.options = options;
        Ok(())
    }
    fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext) {
        if context.width == 0 || context.height == 0 { return; }
        let now = context.elapsed_seconds;
        let scale = (f64::from(context.height) / 32.0).clamp(0.55, 1.25);
        while self.next_burst <= now + 1e-9 {
            self.burst(self.next_burst, scale);
            self.next_burst += self.options.repeat;
        }
        self.last_time = now;
        self.particles.retain(|particle| particle.born + particle.life > now);
        self.pixels.resize(usize::from(context.width) * usize::from(context.height), None);
        self.pixels.fill(None);
        let cx = (f64::from(context.width) - 1.0) / 2.0;
        let cy = (f64::from(context.height) - 1.0) * 0.65;
        for particle in &self.particles {
            let age = now - particle.born;
            if age < 0.0 || age >= particle.life { continue; }
            let drag = (1.0 - (-DRAG * age).exp()) / DRAG;
            let terminal = GRAVITY / DRAG;
            let x = (cx + particle.vx * drag + particle.wobble * (particle.frequency * age + particle.phase).sin() * (age * 3.0).min(1.0)).round() as i32;
            let y = (cy + terminal * age + (particle.vy - terminal) * drag).round() as i32;
            if x < 0 || y < 0 || x >= i32::from(context.width) || y >= i32::from(context.height) { continue; }
            let fraction = age / particle.life;
            let glyph = if fraction > 0.68 { '.' } else if particle.kind == 0 && (age * 8.0 + particle.phase) as i32 % 3 == 0 { '+' } else { [ '*', '+', 'o', 'x' ][particle.kind] };
            let mut color = palette::color(self.options.palette, particle.ink);
            if fraction > 0.8 {
                let fade = ((1.0 - fraction) / 0.2).clamp(0.0, 1.0);
                color = Rgb::new((f64::from(color.r) * fade) as u8, (f64::from(color.g) * fade) as u8, (f64::from(color.b) * fade) as u8);
            }
            let pixel = &mut self.pixels[y as usize * usize::from(context.width) + x as usize];
            // Resolve collisions locally: freshest particle wins; equal ages keep first.
            if pixel.map_or(true, |pixel| particle.born > pixel.priority) {
                *pixel = Some(Pixel { glyph, color, priority: particle.born });
            }
        }
        for (index, pixel) in self.pixels.iter().enumerate() {
            if let Some(pixel) = pixel {
                palette::put(frame, context, (index % usize::from(context.width)) as i32, (index / usize::from(context.width)) as i32, pixel.glyph, pixel.color);
            }
        }
    }
}
