use std::collections::BTreeMap;
use std::f64::consts::PI;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::presets::{OptionDescriptor, OptionValue, PresetDescriptor};
use crate::render::buffer::{Cell, FrameBuffer, Rgb};
use crate::render::{AnimationRenderer, RenderContext};
use crate::{AsciiAnimError, Result};

#[derive(Debug, Clone)]
struct Star {
    arm: i64,
    r: f64,
    angle: f64,
    bright: f64,
    twinkle_phase: f64,
    twinkle_speed: f64,
}

#[derive(Debug, Clone)]
pub struct GalaxyRenderer {
    options: GalaxyOptions,
    stars: Vec<Star>,
}

#[derive(Debug, Clone)]
struct GalaxyOptions {
    arms: i64,
    stars: i64,
    speed: i64,
    size: i64,
    twist: f64,
    noise: f64,
    glow: f64,
    twinkle: f64,
    palette: String,
    gradient: String,
}

pub fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new(
        "galaxy",
        "Rotating Galaxy",
        "A rotating spiral galaxy rendered with ASCII stars",
        vec![
            OptionDescriptor::int("arms", "Arms", 3, 1, 10, true),
            OptionDescriptor::int("stars", "Stars", 600, 100, 1200, true),
            OptionDescriptor::int("speed", "Speed", 20, 1, 60, false),
            OptionDescriptor::int("size", "Size", 70, 20, 100, true),
            OptionDescriptor::float("twist", "Twist", 0.45, 0.0, 1.0, true),
            OptionDescriptor::float("noise", "Noise", 0.15, 0.0, 0.5, true),
            OptionDescriptor::float("glow", "Glow", 0.45, 0.0, 1.0, false),
            OptionDescriptor::float("twinkle", "Twinkle", 0.35, 0.0, 1.0, false),
            OptionDescriptor::choice(
                "palette",
                "Palette",
                "cosmic",
                vec!["cosmic", "stardust", "nebula", "rainbow", "ice", "mono"],
                false,
            ),
            OptionDescriptor::choice(
                "gradient",
                "Gradient",
                "smooth",
                vec!["smooth", "classic", "starry", "block"],
                false,
            ),
        ],
    )
}

pub fn renderer(options: &BTreeMap<String, OptionValue>, seed: u64) -> Result<GalaxyRenderer> {
    let validated = descriptor().validate_options(options)?;
    let options = GalaxyOptions::from_values(&validated)?;
    let mut rng = StdRng::seed_from_u64(seed);
    let stars = build_stars(&options, &mut rng);
    Ok(GalaxyRenderer { options, stars })
}

impl AnimationRenderer for GalaxyRenderer {
    fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext) {
        let rotation = self.options.speed as f64 * PI / 180.0 * context.elapsed_seconds;
        let cx = context.width as f64 / 2.0;
        let cy = context.height as f64 / 2.0;
        let scale = (context.width.min(context.height * 2)) as f64 * 0.46;
        let aspect = 0.48;
        let gradient = gradient(&self.options.gradient);
        let palette = palette(&self.options.palette);

        for star in &self.stars {
            let twinkle = self.options.twinkle
                * 0.5
                * (context.elapsed_seconds * star.twinkle_speed * PI * 2.0
                    + star.twinkle_phase * PI * 2.0)
                    .sin();
            let brightness = (star.bright + twinkle).clamp(0.0, 1.0);
            let angle = star.angle + rotation;
            let px = (cx + angle.cos() * star.r * scale).round() as i32;
            let py = (cy + angle.sin() * star.r * scale * aspect).round() as i32;

            if px < 0 || py < 0 || px >= context.width as i32 || py >= context.height as i32 {
                continue;
            }

            let glow_boost = self.options.glow * 0.4;
            let char_idx = ((brightness + glow_boost) * (gradient.len() as f64 - 0.01)).floor() as usize;
            let ch = gradient[char_idx.min(gradient.len() - 1)];
            let color = if self.options.palette == "rainbow" {
                palette[star.arm as usize % palette.len()]
            } else {
                let color_idx = (brightness * (palette.len() as f64 - 0.01)).floor() as usize;
                palette[color_idx.min(palette.len() - 1)]
            };

            frame.put_cell(
                context.x_offset + px as u16,
                context.y_offset + py as u16,
                Cell::visible(ch, Some(color), context.layer, context.z_index, context.order),
            );
        }
    }
}

impl GalaxyOptions {
    fn from_values(values: &BTreeMap<String, OptionValue>) -> Result<Self> {
        Ok(Self {
            arms: get_int(values, "arms")?,
            stars: get_int(values, "stars")?,
            speed: get_int(values, "speed")?,
            size: get_int(values, "size")?,
            twist: get_float(values, "twist")?,
            noise: get_float(values, "noise")?,
            glow: get_float(values, "glow")?,
            twinkle: get_float(values, "twinkle")?,
            palette: get_choice(
                values,
                "palette",
                &["cosmic", "stardust", "nebula", "rainbow", "ice", "mono"],
            )?,
            gradient: get_choice(values, "gradient", &["smooth", "classic", "starry", "block"])?,
        })
    }
}

fn build_stars(options: &GalaxyOptions, rng: &mut StdRng) -> Vec<Star> {
    let mut stars = Vec::with_capacity(options.stars as usize);
    let twist = options.twist * 4.5;
    let size = options.size as f64 / 100.0;

    for _ in 0..options.stars {
        let arm = rng.gen_range(0..options.arms);
        let r = rng.gen::<f64>().powf(0.55) * size;
        let arm_angle = arm as f64 / options.arms as f64 * PI * 2.0;
        let spiral_angle = arm_angle + r * twist;
        let spread = options.noise * (1.0 - r * 0.5) * PI * 0.6;
        let angle = spiral_angle + (rng.gen::<f64>() - 0.5) * spread * 2.0;
        let nr = r + (rng.gen::<f64>() - 0.5) * options.noise * 0.18;
        let bright = ((1.0 - nr / size).max(0.0) * 0.85) + rng.gen::<f64>() * 0.15;
        stars.push(Star {
            arm,
            r: nr,
            angle,
            bright,
            twinkle_phase: rng.gen(),
            twinkle_speed: 0.5 + rng.gen::<f64>() * 2.5,
        });
    }

    stars
}

fn get_int(values: &BTreeMap<String, OptionValue>, key: &str) -> Result<i64> {
    match values.get(key) {
        Some(OptionValue::Int(value)) => Ok(*value),
        Some(value) => Err(AsciiAnimError::InvalidOptionType {
            option: key.to_string(),
            expected: "integer",
            actual: value.as_cli_value(),
        }),
        None => Err(AsciiAnimError::UnknownOption {
            preset: "galaxy".to_string(),
            option: key.to_string(),
        }),
    }
}

fn get_float(values: &BTreeMap<String, OptionValue>, key: &str) -> Result<f64> {
    match values.get(key) {
        Some(OptionValue::Float(value)) => Ok(*value),
        Some(value) => Err(AsciiAnimError::InvalidOptionType {
            option: key.to_string(),
            expected: "float",
            actual: value.as_cli_value(),
        }),
        None => Err(AsciiAnimError::UnknownOption {
            preset: "galaxy".to_string(),
            option: key.to_string(),
        }),
    }
}

fn get_choice(values: &BTreeMap<String, OptionValue>, key: &str, choices: &[&str]) -> Result<String> {
    match values.get(key) {
        Some(OptionValue::Choice(value)) if choices.contains(&value.as_str()) => Ok(value.clone()),
        Some(OptionValue::Choice(value)) => Err(AsciiAnimError::InvalidChoice {
            option: key.to_string(),
            choices: choices.iter().map(|choice| choice.to_string()).collect(),
            actual: value.clone(),
        }),
        Some(value) => Err(AsciiAnimError::InvalidOptionType {
            option: key.to_string(),
            expected: "choice",
            actual: value.as_cli_value(),
        }),
        None => Err(AsciiAnimError::UnknownOption {
            preset: "galaxy".to_string(),
            option: key.to_string(),
        }),
    }
}

const GRADIENT_CLASSIC: [char; 10] = [' ', '.', ',', '-', '~', '=', '*', '#', '%', '@'];
const GRADIENT_STARRY: [char; 10] = [' ', '·', '✦', '✧', '⁕', '*', '⋆', '★', '✸', '✺'];
const GRADIENT_BLOCK: [char; 10] = [' ', '░', '░', '▒', '▒', '▓', '▓', '█', '█', '■'];
const GRADIENT_SMOOTH: [char; 10] = [' ', '·', ':', ';', '+', 'o', 'O', '0', '@', '#'];

const PALETTE_STARDUST: [Rgb; 7] = [
    Rgb::new(0x1a, 0x14, 0x00),
    Rgb::new(0x4a, 0x38, 0x00),
    Rgb::new(0xaa, 0x88, 0x00),
    Rgb::new(0xdd, 0xbb, 0x00),
    Rgb::new(0xff, 0xee, 0x44),
    Rgb::new(0xff, 0xf4, 0xaa),
    Rgb::new(0xff, 0xff, 0xff),
];
const PALETTE_NEBULA: [Rgb; 7] = [
    Rgb::new(0x20, 0x00, 0x30),
    Rgb::new(0x55, 0x00, 0xaa),
    Rgb::new(0xbb, 0x00, 0xdd),
    Rgb::new(0xee, 0x44, 0xff),
    Rgb::new(0xff, 0x99, 0xff),
    Rgb::new(0xff, 0xcc, 0xff),
    Rgb::new(0xff, 0xff, 0xff),
];
const PALETTE_RAINBOW: [Rgb; 7] = [
    Rgb::new(0xff, 0x22, 0x22),
    Rgb::new(0xff, 0x88, 0x00),
    Rgb::new(0xff, 0xee, 0x00),
    Rgb::new(0x44, 0xdd, 0x44),
    Rgb::new(0x22, 0x99, 0xff),
    Rgb::new(0x99, 0x44, 0xff),
    Rgb::new(0xff, 0x44, 0xcc),
];
const PALETTE_ICE: [Rgb; 7] = [
    Rgb::new(0x00, 0x10, 0x30),
    Rgb::new(0x00, 0x33, 0x88),
    Rgb::new(0x22, 0x66, 0xcc),
    Rgb::new(0x55, 0xaa, 0xee),
    Rgb::new(0xaa, 0xdd, 0xff),
    Rgb::new(0xdd, 0xee, 0xff),
    Rgb::new(0xff, 0xff, 0xff),
];
const PALETTE_MONO: [Rgb; 7] = [
    Rgb::new(0x11, 0x11, 0x11),
    Rgb::new(0x33, 0x33, 0x33),
    Rgb::new(0x55, 0x55, 0x55),
    Rgb::new(0x77, 0x77, 0x77),
    Rgb::new(0x99, 0x99, 0x99),
    Rgb::new(0xbb, 0xbb, 0xbb),
    Rgb::new(0xff, 0xff, 0xff),
];
const PALETTE_COSMIC: [Rgb; 7] = [
    Rgb::new(0x1a, 0x20, 0x60),
    Rgb::new(0x22, 0x33, 0xaa),
    Rgb::new(0x44, 0x66, 0xee),
    Rgb::new(0x66, 0x99, 0xff),
    Rgb::new(0x99, 0xbb, 0xff),
    Rgb::new(0xcc, 0xde, 0xff),
    Rgb::new(0xff, 0xff, 0xff),
];
fn gradient(name: &str) -> &'static [char] {
    match name {
        "classic" => &GRADIENT_CLASSIC,
        "starry" => &GRADIENT_STARRY,
        "block" => &GRADIENT_BLOCK,
        _ => &GRADIENT_SMOOTH,
    }
}

fn palette(name: &str) -> &'static [Rgb] {
    match name {
        "stardust" => &PALETTE_STARDUST,
        "nebula" => &PALETTE_NEBULA,
        "rainbow" => &PALETTE_RAINBOW,
        "ice" => &PALETTE_ICE,
        "mono" => &PALETTE_MONO,
        _ => &PALETTE_COSMIC,
    }
}
