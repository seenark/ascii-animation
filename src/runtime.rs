use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};

use crate::presets::{galaxy, PresetRegistry};
use crate::render::ansi::render_to_ansi;
use crate::render::buffer::FrameBuffer;
use crate::render::layout::resolve_placement;
use crate::render::{AnimationRenderer, RenderContext};
use crate::scene::Scene;
use crate::{AsciiAnimError, Result};

pub fn render_scene_frame(
    scene: &Scene,
    registry: &PresetRegistry,
    seed: u64,
    elapsed_seconds: f64,
    width: u16,
    height: u16,
) -> Result<FrameBuffer> {
    let mut frame = FrameBuffer::new(width, height);
    for (order, instance) in scene.instances.iter().enumerate() {
        if !instance.enabled {
            continue;
        }
        registry.get(&instance.preset)?;
        let rect = resolve_placement(&instance.placement, width, height, width, height);
        if instance.preset == "galaxy" {
            let mut renderer = galaxy::renderer(&instance.options, seed + order as u64)?;
            renderer.render(
                &mut frame,
                RenderContext {
                    elapsed_seconds,
                    layer: instance.layer,
                    z_index: instance.z_index,
                    order,
                    x_offset: rect.x,
                    y_offset: rect.y,
                    width: rect.width,
                    height: rect.height,
                },
            );
        }
    }
    Ok(frame)
}

pub fn run_scene(scene: Scene, registry: &PresetRegistry, seed: u64) -> Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode().map_err(|err| AsciiAnimError::Terminal(err.to_string()))?;
    execute!(stdout, EnterAlternateScreen, Hide)
        .map_err(|err| AsciiAnimError::Terminal(err.to_string()))?;

    let result = run_scene_loop(&mut stdout, scene, registry, seed);

    let restore_result = execute!(stdout, Show, LeaveAlternateScreen)
        .and_then(|_| terminal::disable_raw_mode())
        .map_err(|err| AsciiAnimError::Terminal(err.to_string()));

    result.and(restore_result)
}

fn run_scene_loop(
    stdout: &mut io::Stdout,
    scene: Scene,
    registry: &PresetRegistry,
    seed: u64,
) -> Result<()> {
    let start = Instant::now();
    let frame_duration = Duration::from_millis(1000 / scene.frame_rate.max(1) as u64);

    loop {
        if event::poll(Duration::from_millis(1))
            .map_err(|err| AsciiAnimError::Terminal(err.to_string()))?
        {
            if matches!(
                event::read().map_err(|err| AsciiAnimError::Terminal(err.to_string()))?,
                Event::Key(key)
                    if key.code == KeyCode::Char('q')
                        || key.code == KeyCode::Esc
                        || key.code == KeyCode::Char('c')
            ) {
                break;
            }
        }

        let (width, height) =
            terminal::size().map_err(|err| AsciiAnimError::Terminal(err.to_string()))?;
        let frame = render_scene_frame(
            &scene,
            registry,
            seed,
            start.elapsed().as_secs_f64(),
            width,
            height,
        )?;
        let output = render_to_ansi(&frame, scene.color);
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))
            .map_err(|err| AsciiAnimError::Terminal(err.to_string()))?;
        write!(stdout, "{}", output).map_err(|err| AsciiAnimError::Terminal(err.to_string()))?;
        stdout
            .flush()
            .map_err(|err| AsciiAnimError::Terminal(err.to_string()))?;
        std::thread::sleep(frame_duration);
    }

    Ok(())
}
