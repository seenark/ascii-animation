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

/// Stable live identity. Serialized display identifiers are deliberately not keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryId(u64);

struct RuntimeEntry {
    id: EntryId,
    seed: u64,
    epoch: Duration,
    renderer: Box<dyn crate::render::AnimationRenderer>,
    rect: crate::render::Rect,
    frame: FrameBuffer,
}

pub struct SceneSession {
    scene: Scene,
    registry: PresetRegistry,
    seed: u64,
    next_ordinal: u64,
    elapsed: Duration,
    paused: bool,
    entries: Vec<RuntimeEntry>,
    logical: FrameBuffer,
    viewport: FrameBuffer,
}

impl SceneSession {
    pub fn new(scene: Scene, registry: &PresetRegistry, seed: u64) -> Result<Self> {
        let mut session = Self {
            scene: Scene::default(),
            registry: registry.clone(),
            seed,
            next_ordinal: 0,
            elapsed: Duration::ZERO,
            paused: false,
            entries: Vec::new(),
            logical: FrameBuffer::new(0, 0),
            viewport: FrameBuffer::new(0, 0),
        };
        let identities = vec![None; scene.instances.len()];
        session.apply_scene(scene, &identities)?;
        Ok(session)
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn entry_ids(&self) -> Vec<EntryId> {
        self.entries.iter().map(|entry| entry.id).collect()
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed.as_secs_f64()
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Accept a wall-time delta, discarding stalls instead of retaining backlog.
    pub fn advance(&mut self, delta: Duration) {
        if !self.paused {
            let interval = Duration::from_secs_f64(1.0 / self.scene.frame_rate.max(1) as f64);
            self.elapsed += delta.min(Duration::from_millis(100).max(interval));
        }
    }

    /// Commit an edited Scene. Move identities with instances; use None for new entries.
    /// Returns identities whose simulation intentionally restarted.
    pub fn apply_scene(
        &mut self,
        mut scene: Scene,
        identities: &[Option<EntryId>],
    ) -> Result<Vec<EntryId>> {
        if scene.instances.is_empty() {
            return Err(AsciiAnimError::EmptyScene);
        }
        if identities.len() != scene.instances.len() {
            return Err(AsciiAnimError::Terminal("runtime entry count does not match Scene".into()));
        }
        for (index, identity) in identities.iter().enumerate() {
            if let Some(id) = identity {
                if identities[..index].contains(&Some(*id))
                    || !self.entries.iter().any(|entry| entry.id == *id)
                {
                    return Err(AsciiAnimError::Terminal("invalid live animation identity".into()));
                }
            }
        }
        for instance in &mut scene.instances {
            let descriptor = self.registry.get(&instance.preset)
                .map_err(|error| instance_error(instance, error))?;
            instance.options = descriptor.validate_options(&instance.options)
                .map_err(|error| instance_error(instance, error))?;
        }
        let (width, height) = logical_scene_dimensions(&scene, &self.registry)?;
        let mut replacements = Vec::with_capacity(scene.instances.len());
        let mut next_ordinal = self.next_ordinal;
        let mut restarted = Vec::new();
        for (instance, identity) in scene.instances.iter().zip(identities) {
            let descriptor = self.registry.get(&instance.preset)?;
            let (desired_width, desired_height) = desired_dimensions(instance, width, height);
            let rect = resolve_placement(&instance.placement, width, height, desired_width, desired_height);
            let existing = identity.and_then(|id| self.entries.iter().position(|entry| entry.id == id));
            let needs_rebuild = match existing {
                None => true,
                Some(index) => {
                    let previous = &self.scene.instances[index];
                    let entry = &self.entries[index];
                    previous.preset != instance.preset
                        || descriptor.options().iter().any(|option| {
                            option.rebuilds_state()
                                && previous.options.get(option.name()) != instance.options.get(option.name())
                        })
                        || (entry.renderer.depends_on_dimensions()
                            && (entry.rect.width, entry.rect.height) != (rect.width, rect.height))
                }
            };
            let replacement = if needs_rebuild {
                let (id, seed) = match existing {
                    Some(index) => (self.entries[index].id, self.entries[index].seed),
                    None => {
                        let ordinal = next_ordinal;
                        next_ordinal += 1;
                        (EntryId(ordinal), self.seed.wrapping_add(ordinal))
                    }
                };
                let renderer = descriptor.create_renderer(&instance.options, seed)
                    .map_err(|error| instance_error(instance, error))?;
                restarted.push(id);
                Some(RuntimeEntry {
                    id, seed, epoch: self.elapsed, renderer, rect,
                    frame: FrameBuffer::new(rect.width, rect.height),
                })
            } else {
                None
            };
            replacements.push((existing, rect, replacement));
        }
        // All construction and validation succeeds before touching retained entries.
        // Catch up accepted time using old settings before applying a live change.
        self.render_entries();
        let mut reconfigured: Vec<usize> = Vec::new();
        for (new_index, (existing, _, replacement)) in replacements.iter().enumerate() {
            if replacement.is_none() {
                let index = existing.expect("retained entry has identity");
                if self.scene.instances[index].options != scene.instances[new_index].options {
                    if let Err(error) = self.entries[index].renderer.reconfigure(&scene.instances[new_index].options) {
                        for restored in reconfigured {
                            self.entries[restored].renderer.reconfigure(&self.scene.instances[restored].options)?;
                        }
                        return Err(instance_error(&scene.instances[new_index], error));
                    }
                    reconfigured.push(index);
                }
            }
        }
        let mut previous = std::mem::take(&mut self.entries).into_iter().map(Some).collect::<Vec<_>>();
        self.entries = replacements.into_iter().map(|(existing, rect, replacement)| {
            let mut entry = replacement.unwrap_or_else(|| previous[existing.unwrap()].take().unwrap());
            entry.rect = rect;
            entry
        }).collect();
        self.next_ordinal = next_ordinal;
        self.scene = scene;
        self.logical.reset(width, height);
        Ok(restarted)
    }

    pub fn canvas_dimensions(&self) -> (u16, u16) {
        (self.logical.width(), self.logical.height())
    }

    /// Draw current accepted time. Viewport-only changes do not change simulation state.
    pub fn draw(&mut self, viewport_width: u16, viewport_height: u16) -> Result<&FrameBuffer> {
        self.render_entries();
        center_frame_into(&self.logical, &mut self.viewport, viewport_width, viewport_height);
        Ok(&self.viewport)
    }

    fn render_entries(&mut self) {
        self.logical.reset(self.logical.width(), self.logical.height());
        for (order, (entry, instance)) in self.entries.iter_mut().zip(&self.scene.instances).enumerate() {
            let rect = entry.rect;
            entry.frame.reset(rect.width, rect.height);
            entry.renderer.render(&mut entry.frame, RenderContext {
                elapsed_seconds: self.elapsed.saturating_sub(entry.epoch).as_secs_f64(),
                layer: crate::scene::Layer::Normal, z_index: 0, order: 0,
                x_offset: 0, y_offset: 0, width: rect.width, height: rect.height,
            });
            if instance.enabled {
                for y in 0..rect.height {
                    for x in 0..rect.width {
                        if let Some(cell) = entry.frame.get(x, y) {
                            let mut cell = *cell;
                            cell.layer = instance.layer;
                            cell.z_index = instance.z_index;
                            cell.order = order;
                            self.logical.put_cell(rect.x + x, rect.y + y, cell);
                        }
                    }
                }
            }
        }
    }
}

fn instance_error(instance: &AnimationInstance, error: AsciiAnimError) -> AsciiAnimError {
    AsciiAnimError::AnimationInstance {
        id: instance.id.clone(),
        preset: instance.preset.clone(),
        source: Box::new(error),
    }
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


fn center_frame_into(source: &FrameBuffer, frame: &mut FrameBuffer, viewport_width: u16, viewport_height: u16) {
    frame.reset(viewport_width, viewport_height);
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

}

pub fn scene_viewport_size_for_terminal(
    scene: &Scene,
    registry: &PresetRegistry,
    terminal_width: u16,
    terminal_height: u16,
) -> Result<(u16, u16)> {
    let (logical_width, _) = logical_scene_dimensions(scene, registry)?;
    Ok(viewport_for_canvas(logical_width, terminal_width, terminal_height))
}

fn viewport_for_canvas(logical_width: u16, terminal_width: u16, terminal_height: u16) -> (u16, u16) {
    let (base_width, base_height) =
        animation_viewport_size_for_terminal(terminal_width, terminal_height);
    let expanded_width = if logical_width > DEFAULT_SCENE_WIDTH {
        logical_width.min(terminal_width)
    } else {
        base_width
    };
    (base_width.max(expanded_width), base_height)
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
    let frame_duration = Duration::from_secs_f64(1.0 / scene.frame_rate.max(1) as f64);
    let color = scene.color;
    let mut session = SceneSession::new(scene, registry, seed)?;
    let mut last_tick = Instant::now();

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
            viewport_for_canvas(session.canvas_dimensions().0, terminal_width, terminal_height);
        let now = Instant::now();
        session.advance(now.duration_since(last_tick));
        last_tick = now;
        let frame = session.draw(viewport_width, viewport_height)?;
        let x_offset = terminal_width.saturating_sub(viewport_width) / 2;
        let y_offset = terminal_height.saturating_sub(viewport_height) / 2;
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All)).map_err(terminal_error)?;
        write_positioned_frame(stdout, frame, color, x_offset, y_offset)?;
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
            instances: vec![AnimationInstance {
                id: "galaxy-1".into(), preset: "galaxy".into(),
                options: crate::presets::galaxy::descriptor().defaults(),
                placement: Placement::Center, layer: crate::scene::Layer::Normal,
                z_index: 0, enabled: true,
            }],
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

        assert_eq!(logical_scene_dimensions(&scene, &registry).unwrap().0, 200);
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
