use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};

use crate::presets::PresetRegistry;
use crate::render::ansi::render_to_ansi;
use crate::render::buffer::FrameBuffer;
use crate::render::layout::resolve_placement;
use crate::render::RenderContext;
use crate::scene::{AnimationInstance, Placement, Scene};
use crate::viewport::animation_viewport_size_for_terminal;
use crate::{AsciiAnimError, Result};

pub const DEFAULT_SCENE_WIDTH: u16 = 110;
pub const DEFAULT_SCENE_HEIGHT: u16 = 46;

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
        let (desired_width, desired_height) = desired_dimensions(instance, width, height);
        let rect = resolve_placement(
            &instance.placement,
            width,
            height,
            desired_width,
            desired_height,
        );
        let mut renderer =
            descriptor.create_renderer(&instance.options, seed.wrapping_add(order as u64))?;
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
    Ok(frame)
}

pub fn logical_scene_dimensions(scene: &Scene, registry: &PresetRegistry) -> Result<(u16, u16)> {
    let mut width = DEFAULT_SCENE_WIDTH;
    let height = DEFAULT_SCENE_HEIGHT;

    for instance in scene.instances.iter().filter(|instance| instance.enabled) {
        let descriptor = registry.get(&instance.preset)?;
        let Some(hint) = descriptor.logical_width_hint(&instance.options)? else {
            continue;
        };
        let required_width = match instance.placement {
            Placement::Center | Placement::Left | Placement::Right => hint.saturating_mul(2),
            Placement::Top | Placement::Bottom | Placement::Fill => hint,
            Placement::Custom { .. } => 0,
        };
        width = width.max(required_width);
    }

    Ok((width, height))
}

pub fn render_centered_scene_frame(
    scene: &Scene,
    registry: &PresetRegistry,
    seed: u64,
    elapsed_seconds: f64,
    viewport_width: u16,
    viewport_height: u16,
) -> Result<FrameBuffer> {
    let (logical_width, logical_height) = logical_scene_dimensions(scene, registry)?;
    let logical = render_scene_frame(
        scene,
        registry,
        seed,
        elapsed_seconds,
        logical_width,
        logical_height,
    )?;
    Ok(center_frame(&logical, viewport_width, viewport_height))
}

fn center_frame(source: &FrameBuffer, viewport_width: u16, viewport_height: u16) -> FrameBuffer {
    let mut frame = FrameBuffer::new(viewport_width, viewport_height);
    let copy_width = source.width().min(viewport_width);
    let copy_height = source.height().min(viewport_height);
    let source_x = source.width().saturating_sub(copy_width) / 2;
    let source_y = source.height().saturating_sub(copy_height) / 2;
    let dest_x = viewport_width.saturating_sub(copy_width) / 2;
    let dest_y = viewport_height.saturating_sub(copy_height) / 2;

    for y in 0..copy_height {
        for x in 0..copy_width {
            if let Some(cell) = source.get(source_x + x, source_y + y) {
                if cell.ch != ' ' {
                    frame.put_cell(dest_x + x, dest_y + y, *cell);
                }
            }
        }
    }

    frame
}

pub fn scene_viewport_size_for_terminal(
    scene: &Scene,
    registry: &PresetRegistry,
    terminal_width: u16,
    terminal_height: u16,
) -> Result<(u16, u16)> {
    let (base_width, base_height) =
        animation_viewport_size_for_terminal(terminal_width, terminal_height);
    let (logical_width, _) = logical_scene_dimensions(scene, registry)?;
    let expanded_width = if logical_width > DEFAULT_SCENE_WIDTH {
        logical_width.min(terminal_width)
    } else {
        base_width
    };
    Ok((base_width.max(expanded_width), base_height))
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

fn write_positioned_frame<W: Write>(
    stdout: &mut W,
    frame: &FrameBuffer,
    color: bool,
    x_offset: u16,
    y_offset: u16,
) -> Result<()> {
    let output = render_to_ansi(frame, color);
    for (row, line) in output.lines().enumerate() {
        execute!(stdout, MoveTo(x_offset, y_offset + row as u16)).map_err(terminal_error)?;
        write!(stdout, "{}", line).map_err(terminal_error)?;
    }
    Ok(())
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
        if terminal
            .poll(Duration::from_millis(1))
            .map_err(terminal_error)?
            && should_exit_scene_loop(&terminal.read().map_err(terminal_error)?)
        {
            break;
        }

        let (terminal_width, terminal_height) = terminal.size().map_err(terminal_error)?;
        let (viewport_width, viewport_height) =
            scene_viewport_size_for_terminal(&scene, registry, terminal_width, terminal_height)?;
        let frame = render_centered_scene_frame(
            &scene,
            registry,
            seed,
            start.elapsed().as_secs_f64(),
            viewport_width,
            viewport_height,
        )?;
        let x_offset = terminal_width.saturating_sub(viewport_width) / 2;
        let y_offset = terminal_height.saturating_sub(viewport_height) / 2;
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All)).map_err(terminal_error)?;
        write_positioned_frame(stdout, &frame, scene.color, x_offset, y_offset)?;
        stdout.flush().map_err(terminal_error)?;
        std::thread::sleep(frame_duration);
    }

    Ok(())
}

fn should_exit_scene_loop(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(key)
            if key.code == KeyCode::Char('c')
                && key.modifiers.contains(KeyModifiers::CONTROL)
    )
}

fn desired_dimensions(
    instance: &AnimationInstance,
    frame_width: u16,
    frame_height: u16,
) -> (u16, u16) {
    match &instance.placement {
        Placement::Fill => (frame_width, frame_height),
        Placement::Center => (frame_width / 2, frame_height / 2),
        Placement::Top | Placement::Bottom => (frame_width, frame_height / 2),
        Placement::Left | Placement::Right => (frame_width / 2, frame_height),
        Placement::Custom { width, height, .. } => (*width, *height),
    }
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
        width: u16,
        height: u16,
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
            Ok((self.width, self.height))
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
    fn desired_dimensions_match_placement_regions() {
        let mut options = std::collections::BTreeMap::new();
        options.insert("size".to_string(), crate::presets::OptionValue::Int(20));
        let right = AnimationInstance {
            id: "galaxy-1".to_string(),
            preset: "galaxy".to_string(),
            options,
            placement: Placement::Right,
            layer: crate::scene::Layer::Normal,
            z_index: 0,
            enabled: true,
        };
        let custom = AnimationInstance {
            id: "galaxy-2".to_string(),
            preset: "galaxy".to_string(),
            options: std::collections::BTreeMap::new(),
            placement: Placement::Custom {
                x: 3,
                y: 1,
                width: 7,
                height: 5,
            },
            layer: crate::scene::Layer::Normal,
            z_index: 0,
            enabled: true,
        };

        assert_eq!(desired_dimensions(&right, 40, 16), (20, 16));
        assert_eq!(desired_dimensions(&custom, 40, 16), (7, 5));
    }

    #[test]
    fn plain_c_does_not_exit_scene_loop() {
        let registry = PresetRegistry::default();
        let mut stdout = Vec::new();
        let mut terminal = LoopTerminal {
            events: VecDeque::from([
                key(KeyCode::Char('c'), KeyModifiers::NONE),
                key(KeyCode::Char('q'), KeyModifiers::NONE),
                key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            ]),
            size_calls: 0,
            width: 20,
            height: 8,
        };

        run_scene_loop(&mut stdout, &mut terminal, scene(), &registry, 1).unwrap();

        assert_eq!(terminal.size_calls, 2);
    }

    #[test]
    fn esc_does_not_exit_scene_loop() {
        let registry = PresetRegistry::default();
        let mut stdout = Vec::new();
        let mut terminal = LoopTerminal {
            events: VecDeque::from([
                key(KeyCode::Esc, KeyModifiers::NONE),
                key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            ]),
            size_calls: 0,
            width: 20,
            height: 8,
        };

        run_scene_loop(&mut stdout, &mut terminal, scene(), &registry, 1).unwrap();

        assert_eq!(terminal.size_calls, 1);
    }

    #[derive(Debug)]
    struct FillRenderer;

    impl crate::render::AnimationRenderer for FillRenderer {
        fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext) {
            for y in 0..context.height {
                for x in 0..context.width {
                    frame.put_cell(
                        context.x_offset + x,
                        context.y_offset + y,
                        crate::render::buffer::Cell::visible(
                            '#',
                            None,
                            context.layer,
                            context.z_index,
                            context.order,
                        ),
                    );
                }
            }
        }
    }

    fn fill_renderer(
        _options: &std::collections::BTreeMap<String, crate::presets::OptionValue>,
        _seed: u64,
    ) -> Result<Box<dyn crate::render::AnimationRenderer>> {
        Ok(Box::new(FillRenderer))
    }

    #[test]
    fn run_scene_loop_uses_tui_preview_sized_viewport_centered_in_terminal() {
        let registry = PresetRegistry::new(vec![crate::presets::PresetDescriptor::new(
            "fill",
            "Fill",
            "Fill test renderer",
            vec![],
            fill_renderer,
        )]);
        let scene = Scene {
            frame_rate: 1000,
            color: false,
            instances: vec![AnimationInstance {
                id: "fill-1".to_string(),
                preset: "fill".to_string(),
                options: std::collections::BTreeMap::new(),
                placement: Placement::Fill,
                layer: crate::scene::Layer::Normal,
                z_index: 0,
                enabled: true,
            }],
        };
        let mut stdout = Vec::new();
        let mut terminal = LoopTerminal {
            events: VecDeque::from([
                key(KeyCode::Char('q'), KeyModifiers::NONE),
                key(KeyCode::Char('c'), KeyModifiers::CONTROL),
            ]),
            size_calls: 0,
            width: 120,
            height: 40,
        };

        run_scene_loop(&mut stdout, &mut terminal, scene, &registry, 1).unwrap();
        let output = String::from_utf8(stdout).unwrap();
        let layout = crate::tui::tui_layout(ratatui::layout::Rect::new(0, 0, 120, 40));
        let viewport_width = layout.preview.width.saturating_sub(2).max(1);
        let viewport_height = layout.preview.height.saturating_sub(2).max(1);
        let expected_x = (120 - viewport_width) / 2 + 1;
        let expected_y = (40 - viewport_height) / 2 + 1;
        let expected_move = format!("\u{1b}[{expected_y};{expected_x}H");

        assert!(output.contains(&expected_move));
        assert!(!output.contains("\u{1b}[2;1H"));
    }


    #[test]
    fn logical_scene_dimensions_use_text_art_extend_width_hint_for_center_placement() {
        let registry = PresetRegistry::default();
        let mut options = crate::presets::text_art::descriptor().defaults();
        options.insert(
            "text".to_string(),
            crate::presets::OptionValue::Text("LONG TERMINAL TEXT".to_string()),
        );
        let options = registry
            .get("text-art")
            .unwrap()
            .validate_options(&options)
            .unwrap();
        let scene = Scene {
            frame_rate: 30,
            color: false,
            instances: vec![AnimationInstance {
                id: "text-art-1".to_string(),
                preset: "text-art".to_string(),
                options,
                placement: Placement::Center,
                layer: crate::scene::Layer::Normal,
                z_index: 0,
                enabled: true,
            }],
        };

        assert_eq!(logical_scene_dimensions(&scene, &registry).unwrap().0, 248);
    }

    #[test]
    fn logical_scene_dimensions_do_not_expand_for_text_art_slide_overflow() {
        let registry = PresetRegistry::default();
        let mut options = crate::presets::text_art::descriptor().defaults();
        options.insert(
            "text".to_string(),
            crate::presets::OptionValue::Text("LONG TERMINAL TEXT".to_string()),
        );
        options.insert(
            "text-overflow".to_string(),
            crate::presets::OptionValue::Choice("slide".to_string()),
        );
        let options = registry
            .get("text-art")
            .unwrap()
            .validate_options(&options)
            .unwrap();
        let scene = Scene {
            frame_rate: 30,
            color: false,
            instances: vec![AnimationInstance {
                id: "text-art-1".to_string(),
                preset: "text-art".to_string(),
                options,
                placement: Placement::Center,
                layer: crate::scene::Layer::Normal,
                z_index: 0,
                enabled: true,
            }],
        };

        assert_eq!(
            logical_scene_dimensions(&scene, &registry).unwrap().0,
            DEFAULT_SCENE_WIDTH
        );
    }

    #[test]
    fn ctrl_c_exits_scene_loop() {
        let registry = PresetRegistry::default();
        let mut stdout = Vec::new();
        let mut terminal = LoopTerminal {
            events: VecDeque::from([key(KeyCode::Char('c'), KeyModifiers::CONTROL)]),
            size_calls: 0,
            width: 20,
            height: 8,
        };

        run_scene_loop(&mut stdout, &mut terminal, scene(), &registry, 1).unwrap();

        assert_eq!(terminal.size_calls, 0);
    }
}
