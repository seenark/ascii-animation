use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};

use crate::presets::{galaxy, OptionValue, PresetDescriptor, PresetRegistry};
use crate::render::ansi::render_to_ansi;
use crate::render::buffer::FrameBuffer;
use crate::render::layout::resolve_placement;
use crate::render::{AnimationRenderer, RenderContext};
use crate::scene::{AnimationInstance, Placement, Scene};
use crate::{AsciiAnimError, Result};

pub trait TerminalDriver {
    fn enable_raw_mode(&mut self) -> io::Result<()>;
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn setup_scene_terminal<W: Write>(&mut self, stdout: &mut W) -> io::Result<()>;
    fn restore_scene_terminal<W: Write>(&mut self, stdout: &mut W) -> io::Result<()>;
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;
    fn read(&mut self) -> io::Result<Event>;
    fn size(&mut self) -> io::Result<(u16, u16)>;
}

struct CrosstermDriver;

impl TerminalDriver for CrosstermDriver {
    fn enable_raw_mode(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()
    }

    fn setup_scene_terminal<W: Write>(&mut self, stdout: &mut W) -> io::Result<()> {
        execute!(stdout, EnterAlternateScreen, Hide)
    }

    fn restore_scene_terminal<W: Write>(&mut self, stdout: &mut W) -> io::Result<()> {
        execute!(stdout, Show, LeaveAlternateScreen)
    }

    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }

    fn size(&mut self) -> io::Result<(u16, u16)> {
        terminal::size()
    }
}

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
        let descriptor = registry.get(&instance.preset)?;
        let (desired_width, desired_height) =
            desired_dimensions(instance, descriptor, width, height);
        let rect = resolve_placement(
            &instance.placement,
            width,
            height,
            desired_width,
            desired_height,
        );
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

pub fn prepare_scene_terminal<W: Write, T: TerminalDriver>(
    stdout: &mut W,
    terminal: &mut T,
) -> Result<()> {
    terminal.enable_raw_mode().map_err(terminal_error)?;
    if let Err(err) = terminal.setup_scene_terminal(stdout) {
        return match restore_scene_terminal(stdout, terminal) {
            Ok(()) => Err(terminal_error(err)),
            Err(cleanup_err) => Err(AsciiAnimError::Terminal(format!(
                "{}; additionally failed to restore terminal: {}",
                err, cleanup_err
            ))),
        };
    }
    Ok(())
}

pub fn run_scene(scene: Scene, registry: &PresetRegistry, seed: u64) -> Result<()> {
    let mut stdout = io::stdout();
    let mut terminal = CrosstermDriver;
    prepare_scene_terminal(&mut stdout, &mut terminal)?;

    let result = run_scene_loop(&mut stdout, &mut terminal, scene, registry, seed);
    let restore_result = restore_scene_terminal(&mut stdout, &mut terminal);

    result.and(restore_result)
}

fn run_scene_loop<W: Write, T: TerminalDriver>(
    stdout: &mut W,
    terminal: &mut T,
    scene: Scene,
    registry: &PresetRegistry,
    seed: u64,
) -> Result<()> {
    let start = Instant::now();
    let frame_duration = Duration::from_millis(1000 / scene.frame_rate.max(1) as u64);

    loop {
        if terminal.poll(Duration::from_millis(1)).map_err(terminal_error)? {
            if should_exit_scene_loop(&terminal.read().map_err(terminal_error)?) {
                break;
            }
        }

        let (width, height) = terminal.size().map_err(terminal_error)?;
        let frame = render_scene_frame(
            &scene,
            registry,
            seed,
            start.elapsed().as_secs_f64(),
            width,
            height,
        )?;
        let output = render_to_ansi(&frame, scene.color);
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All)).map_err(terminal_error)?;
        write!(stdout, "{}", output).map_err(terminal_error)?;
        stdout.flush().map_err(terminal_error)?;
        std::thread::sleep(frame_duration);
    }

    Ok(())
}

fn should_exit_scene_loop(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(key)
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('q')
                || (key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL))
    )
}

fn desired_dimensions(
    instance: &AnimationInstance,
    descriptor: &PresetDescriptor,
    frame_width: u16,
    frame_height: u16,
) -> (u16, u16) {
    if matches!(
        instance.placement,
        Placement::Fill | Placement::Custom { .. }
    ) {
        return (frame_width, frame_height);
    }

    let Some(size_percent) = instance
        .options
        .get("size")
        .and_then(int_option)
        .or_else(|| default_int_option(descriptor, "size"))
    else {
        return (frame_width, frame_height);
    };

    (
        scaled_dimension(frame_width, size_percent),
        scaled_dimension(frame_height, size_percent),
    )
}

fn default_int_option(descriptor: &PresetDescriptor, name: &str) -> Option<u16> {
    descriptor
        .options()
        .iter()
        .find(|option| option.name() == name)
        .and_then(|option| int_option(option.default()))
}

fn int_option(value: &OptionValue) -> Option<u16> {
    match value {
        OptionValue::Int(value) => u16::try_from(*value).ok(),
        _ => None,
    }
}

fn scaled_dimension(total: u16, percent: u16) -> u16 {
    ((total as u32 * percent as u32) / 100).max(1) as u16
}

fn restore_scene_terminal<W: Write, T: TerminalDriver>(
    stdout: &mut W,
    terminal: &mut T,
) -> Result<()> {
    let restore_err = terminal.restore_scene_terminal(stdout).err();
    let disable_err = terminal.disable_raw_mode().err();
    match (restore_err, disable_err) {
        (None, None) => Ok(()),
        (Some(err), None) => Err(terminal_error(err)),
        (None, Some(err)) => Err(terminal_error(err)),
        (Some(restore_err), Some(disable_err)) => Err(AsciiAnimError::Terminal(format!(
            "{}; additionally failed to disable raw mode: {}",
            restore_err, disable_err
        ))),
    }
}

fn terminal_error(err: io::Error) -> AsciiAnimError {
    AsciiAnimError::Terminal(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use std::collections::VecDeque;

    struct LoopTerminal {
        events: VecDeque<Event>,
        size_calls: usize,
    }

    impl TerminalDriver for LoopTerminal {
        fn enable_raw_mode(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn disable_raw_mode(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn setup_scene_terminal<W: Write>(&mut self, _stdout: &mut W) -> io::Result<()> {
            Ok(())
        }

        fn restore_scene_terminal<W: Write>(&mut self, _stdout: &mut W) -> io::Result<()> {
            Ok(())
        }

        fn poll(&mut self, _timeout: Duration) -> io::Result<bool> {
            Ok(!self.events.is_empty())
        }

        fn read(&mut self) -> io::Result<Event> {
            self.events
                .pop_front()
                .ok_or_else(|| io::Error::other("no queued event"))
        }

        fn size(&mut self) -> io::Result<(u16, u16)> {
            self.size_calls += 1;
            Ok((20, 8))
        }
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn scene() -> Scene {
        Scene {
            frame_rate: 1000,
            color: false,
            instances: Vec::new(),
        }
    }

    #[test]
    fn plain_c_does_not_exit_scene_loop() {
        let registry = PresetRegistry::default();
        let mut stdout = Vec::new();
        let mut terminal = LoopTerminal {
            events: VecDeque::from([
                key(KeyCode::Char('c'), KeyModifiers::NONE),
                key(KeyCode::Char('q'), KeyModifiers::NONE),
            ]),
            size_calls: 0,
        };

        run_scene_loop(&mut stdout, &mut terminal, scene(), &registry, 1).unwrap();

        assert_eq!(terminal.size_calls, 1);
    }

    #[test]
    fn ctrl_c_exits_scene_loop() {
        let registry = PresetRegistry::default();
        let mut stdout = Vec::new();
        let mut terminal = LoopTerminal {
            events: VecDeque::from([key(KeyCode::Char('c'), KeyModifiers::CONTROL)]),
            size_calls: 0,
        };

        run_scene_loop(&mut stdout, &mut terminal, scene(), &registry, 1).unwrap();

        assert_eq!(terminal.size_calls, 0);
    }
}
