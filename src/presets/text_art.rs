use std::collections::BTreeMap;
use std::f64::consts::PI;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::presets::{OptionDescriptor, OptionValue, PresetDescriptor};
use crate::render::{AnimationRenderer, Cell, FrameBuffer, RenderContext, Rgb};
use crate::{AsciiAnimError, Result};

const PRESET_NAME: &str = "text-art";
const TEXT_MAX_LEN: usize = 64;
const LONG_TEXT_SCROLL_COLUMNS_PER_SECOND: f64 = 6.0;
const DEFAULT_RAIN_COLUMNS: usize = 55;
const STAR_GLYPHS: [char; 5] = ['·', '.', '+', '✦', '*'];
const RAIN_TAIL_GLYPHS: [char; 6] = ['│', '·', ':', ';', '|', 'ー'];
const NOISE_GLYPHS: [char; 4] = ['·', ',', '.', '`'];
const PARTICLE_GLYPHS: [char; 6] = ['*', '·', '+', '✦', '★', '◆'];
const TYPEWRITER_BLINK: [char; 2] = ['_', ' '];

const FONT5: &[(char, [&str; 7])] = &[
    ('A', ["01110", "10001", "10001", "11111", "10001", "10001", "10001"]),
    ('B', ["11110", "10001", "10001", "11110", "10001", "10001", "11110"]),
    ('C', ["01110", "10001", "10000", "10000", "10000", "10001", "01110"]),
    ('D', ["11110", "10001", "10001", "10001", "10001", "10001", "11110"]),
    ('E', ["11111", "10000", "10000", "11110", "10000", "10000", "11111"]),
    ('F', ["11111", "10000", "10000", "11110", "10000", "10000", "10000"]),
    ('G', ["01110", "10001", "10000", "10111", "10001", "10001", "01111"]),
    ('H', ["10001", "10001", "10001", "11111", "10001", "10001", "10001"]),
    ('I', ["01110", "00100", "00100", "00100", "00100", "00100", "01110"]),
    ('J', ["00111", "00010", "00010", "00010", "00010", "10010", "01100"]),
    ('K', ["10001", "10010", "10100", "11000", "10100", "10010", "10001"]),
    ('L', ["10000", "10000", "10000", "10000", "10000", "10000", "11111"]),
    ('M', ["10001", "11011", "10101", "10101", "10001", "10001", "10001"]),
    ('N', ["10001", "11001", "10101", "10011", "10001", "10001", "10001"]),
    ('O', ["01110", "10001", "10001", "10001", "10001", "10001", "01110"]),
    ('P', ["11110", "10001", "10001", "11110", "10000", "10000", "10000"]),
    ('Q', ["01110", "10001", "10001", "10001", "10101", "10010", "01101"]),
    ('R', ["11110", "10001", "10001", "11110", "10100", "10010", "10001"]),
    ('S', ["01111", "10000", "10000", "01110", "00001", "00001", "11110"]),
    ('T', ["11111", "00100", "00100", "00100", "00100", "00100", "00100"]),
    ('U', ["10001", "10001", "10001", "10001", "10001", "10001", "01110"]),
    ('V', ["10001", "10001", "10001", "10001", "10001", "01010", "00100"]),
    ('W', ["10001", "10001", "10001", "10101", "10101", "11011", "10001"]),
    ('X', ["10001", "10001", "01010", "00100", "01010", "10001", "10001"]),
    ('Y', ["10001", "10001", "01010", "00100", "00100", "00100", "00100"]),
    ('Z', ["11111", "00001", "00010", "00100", "01000", "10000", "11111"]),
    ('0', ["01110", "10011", "10101", "11001", "10001", "10001", "01110"]),
    ('1', ["00100", "01100", "00100", "00100", "00100", "00100", "01110"]),
    ('2', ["01110", "10001", "00001", "00110", "01000", "10000", "11111"]),
    ('3', ["11110", "00001", "00001", "01110", "00001", "00001", "11110"]),
    ('4', ["00010", "00110", "01010", "10010", "11111", "00010", "00010"]),
    ('5', ["11111", "10000", "10000", "11110", "00001", "00001", "11110"]),
    ('6', ["01110", "10000", "10000", "11110", "10001", "10001", "01110"]),
    ('7', ["11111", "00001", "00010", "00100", "01000", "01000", "01000"]),
    ('8', ["01110", "10001", "10001", "01110", "10001", "10001", "01110"]),
    ('9', ["01110", "10001", "10001", "01111", "00001", "00001", "01110"]),
    (' ', ["00000", "00000", "00000", "00000", "00000", "00000", "00000"]),
    ('!', ["00100", "00100", "00100", "00100", "00100", "00000", "00100"]),
    ('?', ["01110", "10001", "00001", "00110", "00100", "00000", "00100"]),
    ('.', ["00000", "00000", "00000", "00000", "00000", "00000", "00100"]),
    ('-', ["00000", "00000", "00000", "11111", "00000", "00000", "00000"]),
    ('+', ["00000", "00100", "00100", "11111", "00100", "00100", "00000"]),
    (':', ["00000", "00100", "00000", "00000", "00000", "00100", "00000"]),
];

const PALETTE_COSMIC: [Rgb; 7] = [
    Rgb::new(26, 51, 153),
    Rgb::new(34, 68, 204),
    Rgb::new(68, 136, 255),
    Rgb::new(102, 170, 255),
    Rgb::new(153, 204, 255),
    Rgb::new(204, 238, 255),
    Rgb::new(255, 255, 255),
];
const PALETTE_FIRE: [Rgb; 7] = [
    Rgb::new(85, 0, 0),
    Rgb::new(170, 34, 0),
    Rgb::new(238, 68, 0),
    Rgb::new(255, 136, 0),
    Rgb::new(255, 187, 0),
    Rgb::new(255, 238, 68),
    Rgb::new(255, 255, 255),
];
const PALETTE_NEON: [Rgb; 7] = [
    Rgb::new(0, 51, 0),
    Rgb::new(0, 102, 0),
    Rgb::new(0, 170, 0),
    Rgb::new(0, 221, 0),
    Rgb::new(0, 255, 68),
    Rgb::new(136, 255, 170),
    Rgb::new(204, 255, 221),
];
const PALETTE_GOLD: [Rgb; 7] = [
    Rgb::new(58, 32, 0),
    Rgb::new(122, 68, 0),
    Rgb::new(204, 136, 0),
    Rgb::new(255, 187, 0),
    Rgb::new(255, 221, 68),
    Rgb::new(255, 238, 153),
    Rgb::new(255, 255, 255),
];
const PALETTE_ICE: [Rgb; 7] = [
    Rgb::new(0, 17, 51),
    Rgb::new(0, 34, 102),
    Rgb::new(17, 85, 170),
    Rgb::new(68, 136, 221),
    Rgb::new(136, 187, 255),
    Rgb::new(204, 221, 255),
    Rgb::new(255, 255, 255),
];
const PALETTE_RAINBOW: [Rgb; 7] = [
    Rgb::new(255, 0, 0),
    Rgb::new(255, 136, 0),
    Rgb::new(255, 255, 0),
    Rgb::new(0, 255, 0),
    Rgb::new(0, 136, 255),
    Rgb::new(136, 0, 255),
    Rgb::new(255, 0, 170),
];
const PALETTE_PLASMA: [Rgb; 7] = [
    Rgb::new(34, 0, 68),
    Rgb::new(102, 0, 136),
    Rgb::new(170, 0, 204),
    Rgb::new(238, 68, 255),
    Rgb::new(255, 136, 255),
    Rgb::new(255, 204, 255),
    Rgb::new(255, 255, 255),
];
const PALETTE_MONO: [Rgb; 7] = [
    Rgb::new(51, 51, 51),
    Rgb::new(85, 85, 85),
    Rgb::new(119, 119, 119),
    Rgb::new(153, 153, 153),
    Rgb::new(187, 187, 187),
    Rgb::new(221, 221, 221),
    Rgb::new(255, 255, 255),
];
const PALETTE_RED: [Rgb; 7] = [
    Rgb::new(34, 0, 0),
    Rgb::new(85, 0, 0),
    Rgb::new(136, 0, 0),
    Rgb::new(204, 0, 0),
    Rgb::new(255, 34, 34),
    Rgb::new(255, 102, 102),
    Rgb::new(255, 170, 170),
];
const PALETTE_CANDY: [Rgb; 7] = [
    Rgb::new(255, 102, 170),
    Rgb::new(255, 153, 204),
    Rgb::new(255, 187, 221),
    Rgb::new(170, 221, 255),
    Rgb::new(136, 204, 255),
    Rgb::new(255, 238, 136),
    Rgb::new(255, 255, 255),
];

#[derive(Debug, Clone)]
struct TextArtOptions {
    text: String,
    overflow: String,
    font: String,
    fill: String,
    palette: String,
    effect: String,
    color_mode: String,
    bg: String,
    speed: f64,
    scale: f64,
    amp: f64,
    freq: f64,
    glitch: f64,
    bright: f64,
    spacing: i64,
    voffset: i64,
    drop_shadow: bool,
    block_shadow: bool,
    border: bool,
    glow: bool,
    reflection: bool,
    particles: bool,
    mirror: bool,
}

#[derive(Debug, Clone)]
struct BgStar {
    x: f64,
    y: f64,
    ch: char,
    phase: f64,
    speed: f64,
}

#[derive(Debug, Clone)]
struct RainColumn {
    x: f64,
    y: f64,
    speed: f64,
    len: usize,
}

#[derive(Debug, Clone)]
struct ParticleSeed {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    decay: f64,
    ch: char,
    color_slot: usize,
}

#[derive(Debug, Clone)]
struct BitmapCell {
    ch: char,
    char_index: usize,
    rel_x: f64,
    rel_y: f64,
    gradient_x: f64,
    gradient_y: f64,
}

#[derive(Debug, Clone)]
struct TextBitmap {
    width: usize,
    height: usize,
    cells: Vec<Option<BitmapCell>>,
    visible_chars: usize,
}

#[derive(Debug, Clone, Copy)]
struct AnimSample {
    dx: f64,
    dy: f64,
    alpha: f64,
    glyph: Option<char>,
}

#[derive(Debug, Clone)]
pub struct TextArtRenderer {
    options: TextArtOptions,
    bg_stars: Vec<BgStar>,
    rain_cols: Vec<RainColumn>,
    particle_seeds: Vec<ParticleSeed>,
    seed: u64,
}
pub fn descriptor() -> PresetDescriptor {
    PresetDescriptor::new(
        PRESET_NAME,
        "ASCII Text Art",
        "Animated 5x7 ASCII text art with fonts, palettes, effects, and backgrounds",
        vec![
            OptionDescriptor::text("text", "Text", "HELLO", TEXT_MAX_LEN, true),
            OptionDescriptor::choice(
                "text-overflow",
                "Overflow",
                "extend",
                vec!["extend", "slide"],
                false,
            ),
            OptionDescriptor::choice(
                "text-font",
                "Font",
                "block",
                vec![
                    "block", "bold", "shadow", "outline", "thin", "double", "bubble", "cyber",
                ],
                true,
            ),
            OptionDescriptor::choice(
                "text-fill",
                "Fill Char",
                "auto",
                vec![
                    "auto", "full", "dark", "medium", "light", "square", "circle", "diamond",
                    "triangle", "star", "hash", "at", "cross",
                ],
                true,
            ),
            OptionDescriptor::choice(
                "text-palette",
                "Palette",
                "cosmic",
                vec![
                    "cosmic", "fire", "neon", "gold", "ice", "rainbow", "plasma", "mono", "red",
                    "candy",
                ],
                false,
            ),
            OptionDescriptor::choice(
                "text-effect",
                "Effect",
                "wave",
                vec![
                    "wave",
                    "pulse",
                    "glitch",
                    "scan",
                    "rain",
                    "fire",
                    "matrix",
                    "dissolve",
                    "bounce",
                    "typewriter",
                    "strobe",
                    "neon-flicker",
                ],
                false,
            ),
            OptionDescriptor::choice(
                "text-color-mode",
                "Color Mode",
                "gradient-h",
                vec![
                    "solid",
                    "gradient-h",
                    "gradient-v",
                    "per-char",
                    "wave-color",
                    "random",
                ],
                false,
            ),
            OptionDescriptor::choice(
                "text-bg",
                "Background",
                "stars",
                vec!["none", "stars", "grid", "rain", "noise", "scanlines"],
                true,
            ),
            OptionDescriptor::float("text-speed", "Speed", 1.5, 0.1, 5.0, false),
            OptionDescriptor::float("text-scale", "Scale", 1.0, 0.4, 2.0, true),
            OptionDescriptor::float("text-amp", "Wave Amp", 2.5, 0.0, 8.0, false),
            OptionDescriptor::float("text-freq", "Wave Freq", 1.0, 0.1, 4.0, false),
            OptionDescriptor::float("text-glitch", "Glitch Amt", 0.15, 0.0, 1.0, false),
            OptionDescriptor::float("text-bright", "Brightness", 1.0, 0.2, 1.0, false),
            OptionDescriptor::int("text-spacing", "Spacing", 2, 0, 4, true),
            OptionDescriptor::int("text-voffset", "V-Offset", 0, -10, 10, false),
            OptionDescriptor::bool("text-drop-shadow", "Drop Shadow", false, false),
            OptionDescriptor::bool("text-block-shadow", "Block Shadow", false, true),
            OptionDescriptor::bool("text-border", "Box Border", false, false),
            OptionDescriptor::bool("text-glow", "Glow Effect", true, false),
            OptionDescriptor::bool("text-reflection", "Reflection", false, false),
            OptionDescriptor::bool("text-particles", "Particles", false, true),
            OptionDescriptor::bool("text-mirror", "Mirror", false, false),
        ],
        boxed_renderer,
    )
    .with_logical_width_hint(logical_width_hint)
}

pub fn boxed_renderer(
    options: &BTreeMap<String, OptionValue>,
    seed: u64,
) -> Result<Box<dyn AnimationRenderer>> {
    Ok(Box::new(renderer(options, seed)?))
}


pub fn logical_width_hint(values: &BTreeMap<String, OptionValue>) -> Result<Option<u16>> {
    let options = TextArtOptions::from_values(values)?;
    if options.overflow != "extend" {
        return Ok(None);
    }

    Ok(Some(scaled_text_width(&options)))
}
pub fn renderer(options: &BTreeMap<String, OptionValue>, seed: u64) -> Result<TextArtRenderer> {
    let validated = descriptor().validate_options(options)?;
    let options = TextArtOptions::from_values(&validated)?;
    let mut rng = StdRng::seed_from_u64(seed);
    Ok(TextArtRenderer {
        bg_stars: build_bg_stars(&mut rng),
        rain_cols: build_rain_columns(&mut rng),
        particle_seeds: build_particle_seeds(&mut rng),
        options,
        seed,
    })
}

impl AnimationRenderer for TextArtRenderer {
    fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext) {
        let bitmap = build_text_bitmap(&self.options);
        let scaled_width = ((bitmap.width as f64) * self.options.scale)
            .round()
            .max(1.0) as i32;
        let scaled_height = ((bitmap.height as f64) * self.options.scale)
            .round()
            .max(1.0) as i32;
        let cx = text_origin_x(
            context.width,
            scaled_width,
            &self.options.overflow,
            self.options.speed,
            context.elapsed_seconds,
        );
        let cy = (context.height as i32 - scaled_height) / 2 + self.options.voffset as i32;
        let frame_tick = (context.elapsed_seconds * 60.0).floor() as u64;

        draw_background(
            frame,
            context,
            &self.options,
            &self.bg_stars,
            &self.rain_cols,
            self.seed,
            frame_tick,
        );

        let mut drawn = Vec::new();
        for scaled_row in 0..scaled_height {
            let src_row = (((scaled_row as f64) / self.options.scale).floor() as usize)
                .min(bitmap.height.saturating_sub(1));
            for scaled_col in 0..scaled_width {
                let src_col = (((scaled_col as f64) / self.options.scale).floor() as usize)
                    .min(bitmap.width.saturating_sub(1));
                let Some(cell) = bitmap.get(src_col, src_row) else {
                    continue;
                };
                let sample = anim_offset(
                    &self.options,
                    cell,
                    scaled_col,
                    scaled_row,
                    scaled_width,
                    scaled_height,
                    context.elapsed_seconds,
                    self.seed,
                    frame_tick,
                );
                let alpha = if self.options.glow {
                    (sample.alpha * 1.15).min(1.0)
                } else {
                    sample.alpha.clamp(0.0, 1.0)
                };
                if alpha < 0.05 {
                    continue;
                }
                let px = cx + scaled_col + sample.dx.round() as i32;
                let py = cy + scaled_row + sample.dy.round() as i32;
                if px < 0 || py < 0 || px >= context.width as i32 || py >= context.height as i32 {
                    continue;
                }

                let color_t = color_position(
                    &self.options,
                    cell,
                    scaled_col,
                    scaled_row,
                    scaled_width,
                    scaled_height,
                    bitmap.visible_chars,
                    context.elapsed_seconds,
                    self.seed,
                    frame_tick,
                );
                let color = scale_rgb(
                    palette_color(color_t, &self.options.palette, self.options.bright),
                    alpha,
                );
                let ch = sample.glyph.unwrap_or(cell.ch);

                if self.options.drop_shadow {
                    let sx = px + 1;
                    let sy = py + 1;
                    if sx >= 0
                        && sy >= 0
                        && sx < context.width as i32
                        && sy < context.height as i32
                    {
                        frame.put_cell(
                            context.x_offset + sx as u16,
                            context.y_offset + sy as u16,
                            Cell::visible(
                                '▒',
                                Some(Rgb::new(17, 17, 51)),
                                context.layer,
                                context.z_index,
                                context.order,
                            ),
                        );
                    }
                }

                put_local_cell(frame, context, px, py, ch, Some(color));
                drawn.push((px, py, ch, color, cell.rel_y));

                if self.options.mirror {
                    let mirror_x = context.width as i32 - 1 - px;
                    if mirror_x != px && mirror_x >= 0 && mirror_x < context.width as i32 {
                        put_local_cell(
                            frame,
                            context,
                            mirror_x,
                            py,
                            ch,
                            Some(scale_rgb(color, 0.5)),
                        );
                    }
                }
            }
        }

        if self.options.reflection {
            draw_reflection(frame, context, cy, scaled_height, &drawn);
        }

        if self.options.border {
            draw_border(frame, context, cx, cy, scaled_width, scaled_height);
        }

        if self.options.particles {
            draw_particles(
                frame,
                context,
                &self.options,
                &self.particle_seeds,
                self.seed,
                frame_tick,
                context.elapsed_seconds,
            );
        }
    }
}

impl TextArtOptions {
    fn from_values(values: &BTreeMap<String, OptionValue>) -> Result<Self> {
        Ok(Self {
            text: get_text(values, "text")?,
            overflow: get_choice_or_default(
                values,
                "text-overflow",
                "extend",
                &["extend", "slide"],
            )?,
            font: get_choice(
                values,
                "text-font",
                &[
                    "block", "bold", "shadow", "outline", "thin", "double", "bubble", "cyber",
                ],
            )?,
            fill: get_choice(
                values,
                "text-fill",
                &[
                    "auto", "full", "dark", "medium", "light", "square", "circle", "diamond",
                    "triangle", "star", "hash", "at", "cross",
                ],
            )?,
            palette: get_choice(
                values,
                "text-palette",
                &[
                    "cosmic", "fire", "neon", "gold", "ice", "rainbow", "plasma", "mono", "red",
                    "candy",
                ],
            )?,
            effect: get_choice(
                values,
                "text-effect",
                &[
                    "wave",
                    "pulse",
                    "glitch",
                    "scan",
                    "rain",
                    "fire",
                    "matrix",
                    "dissolve",
                    "bounce",
                    "typewriter",
                    "strobe",
                    "neon-flicker",
                ],
            )?,
            color_mode: get_choice(
                values,
                "text-color-mode",
                &[
                    "solid",
                    "gradient-h",
                    "gradient-v",
                    "per-char",
                    "wave-color",
                    "random",
                ],
            )?,
            bg: get_choice(
                values,
                "text-bg",
                &["none", "stars", "grid", "rain", "noise", "scanlines"],
            )?,
            speed: get_float(values, "text-speed")?,
            scale: get_float(values, "text-scale")?,
            amp: get_float(values, "text-amp")?,
            freq: get_float(values, "text-freq")?,
            glitch: get_float(values, "text-glitch")?,
            bright: get_float(values, "text-bright")?,
            spacing: get_int(values, "text-spacing")?,
            voffset: get_int(values, "text-voffset")?,
            drop_shadow: get_bool(values, "text-drop-shadow")?,
            block_shadow: get_bool(values, "text-block-shadow")?,
            border: get_bool(values, "text-border")?,
            glow: get_bool(values, "text-glow")?,
            reflection: get_bool(values, "text-reflection")?,
            particles: get_bool(values, "text-particles")?,
            mirror: get_bool(values, "text-mirror")?,
        })
    }
}

impl TextBitmap {
    fn blank(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![None; width * height],
            visible_chars: 1,
        }
    }

    fn set(&mut self, x: usize, y: usize, cell: BitmapCell) {
        self.cells[y * self.width + x] = Some(cell);
    }

    fn get(&self, x: usize, y: usize) -> Option<&BitmapCell> {
        self.cells[y * self.width + x].as_ref()
    }
}

fn text_bitmap_width(options: &TextArtOptions) -> usize {
    let text_len = options.text.chars().count();
    if text_len == 0 {
        return 5;
    }
    text_len * 5 + text_len.saturating_sub(1) * options.spacing.max(0) as usize
}

fn scaled_text_width(options: &TextArtOptions) -> u16 {
    ((text_bitmap_width(options) as f64) * options.scale)
        .round()
        .max(1.0)
        .min(u16::MAX as f64) as u16
}

fn build_text_bitmap(options: &TextArtOptions) -> TextBitmap {
    let text: Vec<char> = options.text.chars().map(normalize_char).collect();
    if text.is_empty() {
        return TextBitmap::blank(5, 7);
    }

    let width = text_bitmap_width(options);
    let mut bitmap = TextBitmap::blank(width.max(5), 7);
    bitmap.visible_chars = text.iter().filter(|ch| **ch != ' ').count().max(1);
    let mut offset_x = 0usize;
    let mut min_x = bitmap.width;
    let mut max_x = 0usize;
    let mut min_y = bitmap.height;
    let mut max_y = 0usize;

    for (char_index, ch) in text.iter().enumerate() {
        let pixels = glyph_pixels(*ch);
        let styled = apply_font_style(
            &pixels,
            &options.font,
            fill_char(&options.font, &options.fill),
            options.block_shadow,
        );
        for (row, row_cells) in styled.iter().enumerate() {
            for (col, pixel) in row_cells.iter().enumerate() {
                if *pixel == ' ' {
                    continue;
                }
                let x = offset_x + col;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(row);
                max_y = max_y.max(row);
                bitmap.set(
                    x,
                    row,
                    BitmapCell {
                        ch: *pixel,
                        char_index,
                        rel_x: col as f64 / 4.0,
                        rel_y: row as f64 / 6.0,
                        gradient_x: 0.0,
                        gradient_y: 0.0,
                    },
                );
            }
        }
        offset_x += 5 + options.spacing.max(0) as usize;
    }

    if min_x <= max_x && min_y <= max_y {
        let x_span = max_x.saturating_sub(min_x).max(1) as f64;
        let y_span = max_y.saturating_sub(min_y).max(1) as f64;
        let width = bitmap.width;
        for (index, cell) in bitmap.cells.iter_mut().enumerate() {
            let Some(cell) = cell.as_mut() else {
                continue;
            };
            let x = index % width;
            let y = index / width;
            cell.gradient_x = (x.saturating_sub(min_x)) as f64 / x_span;
            cell.gradient_y = (y.saturating_sub(min_y)) as f64 / y_span;
        }
    }

    bitmap
}

fn text_origin_x(
    context_width: u16,
    scaled_width: i32,
    overflow: &str,
    speed: f64,
    elapsed_seconds: f64,
) -> i32 {
    let context_width = context_width as i32;
    if scaled_width <= context_width {
        return ((context_width - scaled_width) / 2).max(0);
    }

    if overflow == "extend" {
        return 0;
    }

    let overhang = scaled_width - context_width;
    let columns_per_second = (speed * LONG_TEXT_SCROLL_COLUMNS_PER_SECOND).max(1.0);
    let phase = (elapsed_seconds * columns_per_second).floor() as i32;
    -phase.rem_euclid(overhang + 1)
}

fn apply_font_style(
    pixels: &[[bool; 5]; 7],
    font: &str,
    fill: char,
    block_shadow: bool,
) -> [[char; 5]; 7] {
    let mut out = [[' '; 5]; 7];
    match font {
        "block" | "bold" => {
            fill_pixels(pixels, &mut out, fill);
            if block_shadow {
                apply_block_shadow(pixels, &mut out);
            }
        }
        "shadow" => {
            fill_pixels(pixels, &mut out, fill);
            apply_block_shadow(pixels, &mut out);
        }
        "outline" => outline_pixels(pixels, &mut out, ['┌', '┐', '└', '┘', '─', '│', '·']),
        "thin" => outline_pixels(pixels, &mut out, ['╭', '╮', '╰', '╯', '─', '│', ' ']),
        "double" => outline_pixels(pixels, &mut out, ['╔', '╗', '╚', '╝', '═', '║', '·']),
        "cyber" => outline_pixels(pixels, &mut out, ['◤', '◥', '◣', '◢', '━', '┃', '▸']),
        "bubble" => bubble_pixels(pixels, &mut out),
        _ => fill_pixels(pixels, &mut out, fill),
    }
    out
}

fn apply_block_shadow(pixels: &[[bool; 5]; 7], out: &mut [[char; 5]; 7]) {
    for y in 0..7 {
        for x in 0..5 {
            if pixels[y][x] {
                continue;
            }
            if (y > 0 && pixels[y - 1][x]) || (x > 0 && pixels[y][x - 1]) {
                out[y][x] = '▒';
            }
        }
    }
}

fn fill_pixels(pixels: &[[bool; 5]; 7], out: &mut [[char; 5]; 7], ch: char) {
    for y in 0..7 {
        for x in 0..5 {
            if pixels[y][x] {
                out[y][x] = ch;
            }
        }
    }
}

fn outline_pixels(pixels: &[[bool; 5]; 7], out: &mut [[char; 5]; 7], chars: [char; 7]) {
    for y in 0..7 {
        for x in 0..5 {
            if !pixels[y][x] {
                continue;
            }
            let up = y > 0 && pixels[y - 1][x];
            let down = y < 6 && pixels[y + 1][x];
            let left = x > 0 && pixels[y][x - 1];
            let right = x < 4 && pixels[y][x + 1];
            out[y][x] = match (up, down, left, right) {
                (false, true, false, true) => chars[0],
                (false, true, true, false) => chars[1],
                (true, false, false, true) => chars[2],
                (true, false, true, false) => chars[3],
                (true, true, false, false) => chars[5],
                (false, false, true, true) => chars[4],
                (false, true, true, true) | (true, false, true, true) => chars[4],
                (true, true, true, false) | (true, true, false, true) => chars[5],
                _ => chars[6],
            };
        }
    }
}

fn bubble_pixels(pixels: &[[bool; 5]; 7], out: &mut [[char; 5]; 7]) {
    for y in 0..7 {
        for x in 0..5 {
            if !pixels[y][x] {
                continue;
            }
            let up = y > 0 && pixels[y - 1][x];
            let down = y < 6 && pixels[y + 1][x];
            let left = x > 0 && pixels[y][x - 1];
            let right = x < 4 && pixels[y][x + 1];
            out[y][x] = match (up, down, left, right) {
                (false, true, false, true) => '(',
                (false, true, true, false) => ')',
                (true, false, false, true) => '(',
                (true, false, true, false) => ')',
                (true, true, false, false) => '|',
                (false, false, true, true) => '─',
                (true, true, true, true) => '◉',
                (false, false, false, false) => '●',
                (false, true, true, true) => '◠',
                (true, false, true, true) => '◡',
                _ => '◎',
            };
        }
    }
}

fn draw_background(
    frame: &mut FrameBuffer,
    context: RenderContext,
    options: &TextArtOptions,
    bg_stars: &[BgStar],
    rain_cols: &[RainColumn],
    seed: u64,
    frame_tick: u64,
) {
    match options.bg.as_str() {
        "none" => {}
        "stars" => {
            for star in bg_stars {
                let x = ((context.width.saturating_sub(1)) as f64 * star.x).round() as i32;
                let y = ((context.height.saturating_sub(1)) as f64 * star.y).round() as i32;
                let bri = 0.35
                    + 0.65
                        * ((context.elapsed_seconds * star.speed + star.phase).sin() * 0.5 + 0.5);
                if bri < 0.2 {
                    continue;
                }
                put_local_if_empty(
                    frame,
                    context,
                    x,
                    y,
                    star.ch,
                    Some(Rgb::new((100.0 * bri) as u8, (140.0 * bri) as u8, 255)),
                );
            }
        }
        "grid" => {
            for y in 0..context.height as i32 {
                for x in 0..context.width as i32 {
                    let on_row = y % 4 == 0;
                    let on_col = x % 8 == 0;
                    if !on_row && !on_col {
                        continue;
                    }
                    let ch = if on_row && on_col {
                        '+'
                    } else if on_row {
                        '─'
                    } else {
                        '│'
                    };
                    put_local_if_empty(frame, context, x, y, ch, Some(Rgb::new(26, 34, 64)));
                }
            }
        }
        "rain" => {
            for (index, col) in rain_cols.iter().enumerate() {
                let x = ((context.width.saturating_sub(1)) as f64 * col.x).round() as i32;
                let head =
                    ((col.y + context.elapsed_seconds * col.speed * 0.18 + index as f64 * 0.013)
                        .rem_euclid(1.2)
                        - 0.2)
                        * context.height as f64;
                for depth in 0..col.len {
                    let y = head.round() as i32 - depth as i32;
                    if y < 0 || y >= context.height as i32 {
                        continue;
                    }
                    let fade = 1.0 - depth as f64 / col.len.max(1) as f64;
                    let ch = if depth == 0 {
                        '│'
                    } else {
                        pick_hash(
                            &RAIN_TAIL_GLYPHS,
                            seed,
                            x as u64,
                            y as u64,
                            frame_tick,
                            depth as u64,
                        )
                    };
                    put_local_if_empty(
                        frame,
                        context,
                        x,
                        y,
                        ch,
                        Some(Rgb::new(
                            0,
                            (120.0 * fade + 40.0) as u8,
                            (40.0 * fade) as u8,
                        )),
                    );
                }
            }
        }
        "noise" => {
            for y in 0..context.height as i32 {
                for x in 0..context.width as i32 {
                    if unit_hash(seed, x as u64, y as u64, frame_tick, 77) < 0.03 {
                        let ch = pick_hash(&NOISE_GLYPHS, seed, x as u64, y as u64, frame_tick, 79);
                        put_local_if_empty(frame, context, x, y, ch, Some(Rgb::new(26, 26, 42)));
                    }
                }
            }
        }
        "scanlines" => {
            for y in (0..context.height as i32).step_by(2) {
                for x in 0..context.width as i32 {
                    put_local_if_empty(frame, context, x, y, '░', Some(Rgb::new(10, 10, 24)));
                }
            }
        }
        _ => {}
    }
}

fn draw_border(
    frame: &mut FrameBuffer,
    context: RenderContext,
    cx: i32,
    cy: i32,
    scaled_width: i32,
    scaled_height: i32,
) {
    let left = cx - 2;
    let top = cy - 2;
    let right = cx + scaled_width + 1;
    let bottom = cy + scaled_height + 1;
    for x in left..=right {
        let ch_top = if x == left {
            '╔'
        } else if x == right {
            '╗'
        } else {
            '═'
        };
        let ch_bottom = if x == left {
            '╚'
        } else if x == right {
            '╝'
        } else {
            '═'
        };
        let color = if x == left || x == right {
            Some(Rgb::new(68, 102, 204))
        } else {
            Some(Rgb::new(34, 68, 170))
        };
        put_local_cell(frame, context, x, top, ch_top, color);
        put_local_cell(frame, context, x, bottom, ch_bottom, color);
    }
    for y in (top + 1)..bottom {
        put_local_cell(frame, context, left, y, '║', Some(Rgb::new(34, 68, 170)));
        put_local_cell(frame, context, right, y, '║', Some(Rgb::new(34, 68, 170)));
    }
}

fn draw_reflection(
    frame: &mut FrameBuffer,
    context: RenderContext,
    cy: i32,
    scaled_height: i32,
    drawn: &[(i32, i32, char, Rgb, f64)],
) {
    let max_rows = 4.min(scaled_height.max(0) as usize);
    for &(px, py, _ch, color, rel_y) in drawn {
        let local_row = py - cy;
        if local_row < 0 || local_row as usize >= max_rows {
            continue;
        }
        let reflection_y = cy + scaled_height + local_row;
        let depth = local_row as usize;
        let glyph = match depth {
            0 => '▄',
            1 => '░',
            _ => continue,
        };
        let fade = if rel_y < 0.5 { 0.35 } else { 0.22 };
        put_local_cell(
            frame,
            context,
            px,
            reflection_y,
            glyph,
            Some(scale_rgb(color, fade)),
        );
    }
}

fn draw_particles(
    frame: &mut FrameBuffer,
    context: RenderContext,
    options: &TextArtOptions,
    particle_seeds: &[ParticleSeed],
    seed: u64,
    frame_tick: u64,
    elapsed_seconds: f64,
) {
    for (index, particle) in particle_seeds.iter().enumerate() {
        if unit_hash(seed, index as u64, frame_tick, 0, 133) >= 0.15 {
            continue;
        }
        let age = (elapsed_seconds * options.speed * 0.4 + index as f64 * 0.031).fract();
        let x = ((particle.x + particle.vx * age).fract() * context.width as f64).round() as i32;
        let y = ((particle.y - particle.vy * age).fract() * context.height as f64).round() as i32;
        let alpha = (1.0 - age * particle.decay).clamp(0.1, 1.0);
        let color = scale_rgb(
            palette_color(
                particle.color_slot as f64 / 4.0,
                &options.palette,
                options.bright,
            ),
            alpha,
        );
        put_local_if_empty(frame, context, x, y, particle.ch, Some(color));
    }
}

fn color_position(
    options: &TextArtOptions,
    cell: &BitmapCell,
    scaled_col: i32,
    scaled_row: i32,
    _scaled_width: i32,
    _scaled_height: i32,
    visible_text_len: usize,
    elapsed_seconds: f64,
    seed: u64,
    frame_tick: u64,
) -> f64 {
    match options.color_mode.as_str() {
        "solid" => 0.85,
        "gradient-h" => cell.gradient_x,
        "gradient-v" => cell.gradient_y,
        "per-char" => cell.char_index as f64 / visible_text_len.saturating_sub(1).max(1) as f64,
        "wave-color" => {
            ((scaled_col as f64 * 0.3 + elapsed_seconds * options.speed).sin() + 1.0) / 2.0
        }
        "random" => unit_hash(seed, scaled_col as u64, scaled_row as u64, frame_tick, 41),
        _ => 0.85,
    }
}

fn anim_offset(
    options: &TextArtOptions,
    cell: &BitmapCell,
    scaled_col: i32,
    scaled_row: i32,
    scaled_width: i32,
    scaled_height: i32,
    elapsed_seconds: f64,
    seed: u64,
    frame_tick: u64,
) -> AnimSample {
    let speed = options.speed;
    let phase = scaled_col as f64 * options.freq * 0.35 + elapsed_seconds * speed * 2.0;
    let mut sample = AnimSample {
        dx: 0.0,
        dy: 0.0,
        alpha: 1.0,
        glyph: None,
    };

    match options.effect.as_str() {
        "wave" => sample.dy = options.amp * phase.sin(),
        "pulse" => sample.alpha = 0.55 + 0.45 * (elapsed_seconds * speed * 1.4).sin().abs(),
        "scan" => {
            let band = ((elapsed_seconds * speed * 6.0) as i32).rem_euclid(scaled_height.max(1));
            sample.alpha = if (scaled_row - band).abs() <= 1 {
                1.0
            } else {
                0.35
            };
        }
        "bounce" => {
            sample.dy = -options.amp * (elapsed_seconds * speed + cell.rel_x * PI).sin().abs()
        }
        "glitch" => {
            if unit_hash(seed, scaled_col as u64, scaled_row as u64, frame_tick, 7) < options.glitch
            {
                sample.dx = (unit_hash(seed, scaled_col as u64, scaled_row as u64, frame_tick, 8)
                    * 4.0)
                    .round()
                    - 2.0;
                sample.glyph = Some(pick_hash(
                    &['#', '%', '&', '@', 'X'],
                    seed,
                    scaled_col as u64,
                    scaled_row as u64,
                    frame_tick,
                    9,
                ));
            }
            sample.alpha = 0.6 + 0.4 * (1.0 - options.glitch);
        }
        "fire" => {
            sample.dy = -options.amp
                * (unit_hash(seed, scaled_col as u64, scaled_row as u64, frame_tick, 11) * 1.2
                    + cell.rel_y);
            sample.alpha = 0.5 + 0.5 * (1.0 - cell.rel_y);
        }
        "matrix" => {
            sample.dy = ((elapsed_seconds * speed * 3.0 + cell.rel_x * 5.0).sin() + 1.0) * 0.5;
            sample.glyph = Some(pick_hash(
                &['0', '1', '¦', '|', '░'],
                seed,
                scaled_col as u64,
                scaled_row as u64,
                frame_tick,
                12,
            ));
            sample.alpha = 0.45 + 0.55 * (1.0 - cell.rel_y * 0.6);
        }
        "dissolve" => {
            let threshold = ((elapsed_seconds * speed * 0.8).sin() + 1.0) / 2.0;
            sample.alpha = if unit_hash(seed, scaled_col as u64, scaled_row as u64, frame_tick, 13)
                < threshold
            {
                1.0
            } else {
                0.0
            };
        }
        "strobe" => {
            sample.alpha = if ((elapsed_seconds * speed * 6.0).floor() as i64) % 2 == 0 {
                1.0
            } else {
                0.15
            }
        }
        "neon-flicker" => {
            sample.alpha =
                0.7 + 0.3 * unit_hash(seed, scaled_col as u64, scaled_row as u64, frame_tick, 14);
        }
        "typewriter" => {
            let reveal = (elapsed_seconds * speed * 3.0).floor() as usize;
            if cell.char_index > reveal {
                sample.alpha = 0.0;
            } else if cell.char_index == reveal
                && unit_hash(seed, frame_tick, cell.char_index as u64, 0, 15) > 0.5
            {
                sample.glyph =
                    Some(TYPEWRITER_BLINK[(frame_tick as usize) % TYPEWRITER_BLINK.len()]);
            }
        }
        "rain" => {
            sample.dy =
                (elapsed_seconds * speed * 4.0 + cell.rel_x * 2.0).sin() * options.amp * 0.3;
            sample.alpha = 0.5 + 0.5 * (1.0 - cell.rel_y);
        }
        _ => {}
    }

    if options.effect == "wave" && options.amp == 0.0 {
        sample.dy = 0.0;
    }
    if scaled_width <= 1 {
        sample.dx = 0.0;
    }
    sample
}

fn palette_color(t01: f64, palette_name: &str, bright: f64) -> Rgb {
    let palette = palette(palette_name);
    let t = t01.clamp(0.0, 1.0);
    let bright = bright.clamp(0.0, 1.0);
    if palette.len() == 1 {
        return scale_rgb(palette[0], bright);
    }
    let scaled = t * (palette.len() - 1) as f64;
    let idx = scaled.floor() as usize;
    let next = (idx + 1).min(palette.len() - 1);
    let frac = scaled - idx as f64;
    let lerp = |a: u8, b: u8| ((a as f64) + (b as f64 - a as f64) * frac).round() as u8;
    scale_rgb(
        Rgb::new(
            lerp(palette[idx].r, palette[next].r),
            lerp(palette[idx].g, palette[next].g),
            lerp(palette[idx].b, palette[next].b),
        ),
        bright,
    )
}

fn build_bg_stars(rng: &mut StdRng) -> Vec<BgStar> {
    (0..90)
        .map(|_| BgStar {
            x: rng.gen(),
            y: rng.gen(),
            ch: STAR_GLYPHS[rng.gen_range(0..STAR_GLYPHS.len())],
            phase: rng.gen_range(0.0..(PI * 2.0)),
            speed: rng.gen_range(0.5..2.0),
        })
        .collect()
}

fn build_rain_columns(rng: &mut StdRng) -> Vec<RainColumn> {
    let count = DEFAULT_RAIN_COLUMNS.max(1);
    (0..count)
        .map(|_| RainColumn {
            x: rng.gen(),
            y: rng.gen(),
            speed: rng.gen_range(0.3..1.5),
            len: rng.gen_range(3..=10),
        })
        .collect()
}

fn build_particle_seeds(rng: &mut StdRng) -> Vec<ParticleSeed> {
    (0..128)
        .map(|_| ParticleSeed {
            x: rng.gen(),
            y: rng.gen(),
            vx: rng.gen_range(-0.08..0.08),
            vy: rng.gen_range(0.05..0.2),
            decay: rng.gen_range(0.7..1.3),
            ch: PARTICLE_GLYPHS[rng.gen_range(0..PARTICLE_GLYPHS.len())],
            color_slot: rng.gen_range(0..5),
        })
        .collect()
}

fn glyph_pixels(ch: char) -> [[bool; 5]; 7] {
    let rows = FONT5
        .iter()
        .find(|(candidate, _)| *candidate == ch)
        .map(|(_, rows)| *rows)
        .unwrap_or(["00000"; 7]);
    let mut out = [[false; 5]; 7];
    for (y, row) in rows.iter().enumerate() {
        for (x, byte) in row.as_bytes().iter().enumerate() {
            out[y][x] = *byte == b'1';
        }
    }
    out
}

fn normalize_char(ch: char) -> char {
    let upper = ch.to_ascii_uppercase();
    if FONT5.iter().any(|(candidate, _)| *candidate == upper) {
        upper
    } else {
        ' '
    }
}

fn fill_char(font: &str, fill: &str) -> char {
    match fill {
        "full" => '█',
        "dark" => '▓',
        "medium" => '▒',
        "light" => '░',
        "square" => '■',
        "circle" => '●',
        "diamond" => '◆',
        "triangle" => '▲',
        "star" => '★',
        "hash" => '#',
        "at" => '@',
        "cross" => 'X',
        _ => match font {
            "block" => '█',
            "bold" => '▓',
            "shadow" => '░',
            "outline" => '+',
            "thin" => '·',
            "double" => '═',
            "bubble" => '●',
            "cyber" => '▲',
            _ => '█',
        },
    }
}

fn palette(name: &str) -> &'static [Rgb] {
    match name {
        "cosmic" => &PALETTE_COSMIC,
        "fire" => &PALETTE_FIRE,
        "neon" => &PALETTE_NEON,
        "gold" => &PALETTE_GOLD,
        "ice" => &PALETTE_ICE,
        "rainbow" => &PALETTE_RAINBOW,
        "plasma" => &PALETTE_PLASMA,
        "mono" => &PALETTE_MONO,
        "red" => &PALETTE_RED,
        "candy" => &PALETTE_CANDY,
        _ => &PALETTE_COSMIC,
    }
}

fn get_text(values: &BTreeMap<String, OptionValue>, key: &str) -> Result<String> {
    match values.get(key) {
        Some(OptionValue::Text(value)) => Ok(value.clone()),
        Some(value) => Err(AsciiAnimError::InvalidOptionType {
            option: key.to_string(),
            expected: "text",
            actual: value.as_cli_value(),
        }),
        None => Err(AsciiAnimError::UnknownOption {
            preset: PRESET_NAME.to_string(),
            option: key.to_string(),
        }),
    }
}

fn get_choice(
    values: &BTreeMap<String, OptionValue>,
    key: &str,
    choices: &[&str],
) -> Result<String> {
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
            preset: PRESET_NAME.to_string(),
            option: key.to_string(),
        }),
    }
}

fn get_choice_or_default(
    values: &BTreeMap<String, OptionValue>,
    key: &str,
    default: &str,
    choices: &[&str],
) -> Result<String> {
    if values.contains_key(key) {
        get_choice(values, key, choices)
    } else {
        Ok(default.to_string())
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
            preset: PRESET_NAME.to_string(),
            option: key.to_string(),
        }),
    }
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
            preset: PRESET_NAME.to_string(),
            option: key.to_string(),
        }),
    }
}

fn get_bool(values: &BTreeMap<String, OptionValue>, key: &str) -> Result<bool> {
    match values.get(key) {
        Some(OptionValue::Bool(value)) => Ok(*value),
        Some(value) => Err(AsciiAnimError::InvalidOptionType {
            option: key.to_string(),
            expected: "bool",
            actual: value.as_cli_value(),
        }),
        None => Err(AsciiAnimError::UnknownOption {
            preset: PRESET_NAME.to_string(),
            option: key.to_string(),
        }),
    }
}

fn scale_rgb(color: Rgb, alpha: f64) -> Rgb {
    let alpha = alpha.clamp(0.0, 1.0);
    Rgb::new(
        (color.r as f64 * alpha).round() as u8,
        (color.g as f64 * alpha).round() as u8,
        (color.b as f64 * alpha).round() as u8,
    )
}

fn put_local_cell(
    frame: &mut FrameBuffer,
    context: RenderContext,
    x: i32,
    y: i32,
    ch: char,
    color: Option<Rgb>,
) {
    if x < 0 || y < 0 || x >= context.width as i32 || y >= context.height as i32 {
        return;
    }
    frame.put_cell(
        context.x_offset + x as u16,
        context.y_offset + y as u16,
        Cell::visible(ch, color, context.layer, context.z_index, context.order),
    );
}

fn put_local_if_empty(
    frame: &mut FrameBuffer,
    context: RenderContext,
    x: i32,
    y: i32,
    ch: char,
    color: Option<Rgb>,
) {
    if x < 0 || y < 0 || x >= context.width as i32 || y >= context.height as i32 {
        return;
    }
    let gx = context.x_offset + x as u16;
    let gy = context.y_offset + y as u16;
    if frame.get(gx, gy).is_some_and(|cell| cell.ch != ' ') {
        return;
    }
    frame.put_cell(
        gx,
        gy,
        Cell::visible(ch, color, context.layer, context.z_index, context.order),
    );
}

fn unit_hash(seed: u64, a: u64, b: u64, c: u64, salt: u64) -> f64 {
    let mut value = seed
        .wrapping_add(a.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .wrapping_add(c.wrapping_mul(0x1656_67B1_9E37_79F9))
        .wrapping_add(salt.wrapping_mul(0x85EB_CA6B));
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^= value >> 33;
    (value as f64) / (u64::MAX as f64)
}

fn pick_hash<const N: usize>(
    choices: &[char; N],
    seed: u64,
    a: u64,
    b: u64,
    c: u64,
    salt: u64,
) -> char {
    let idx = (unit_hash(seed, a, b, c, salt) * N as f64).floor() as usize;
    choices[idx.min(N - 1)]
}
