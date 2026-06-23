use std::cell::RefCell;
use std::io;
use std::time::{Duration, Instant};

use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use crate::presets::{OptionKind, OptionValue, PresetRegistry};
use crate::render::buffer::FrameBuffer;
use crate::runtime::render_scene_frame;
use crate::scene::{AnimationInstance, Layer, Placement, Scene};
use crate::{AsciiAnimError, Result};

pub struct TuiState {
    pub scene: Scene,
    selected_instance: usize,
    selected_option: usize,
    option_names: Vec<String>,
    option_kinds: Vec<OptionKind>,
    custom_placements: Vec<Placement>,
    last_exported_scene: RefCell<Option<Scene>>,
}

const CUSTOM_X_OPTION: &str = "placement-x";
const CUSTOM_Y_OPTION: &str = "placement-y";
const CUSTOM_WIDTH_OPTION: &str = "placement-width";
const CUSTOM_HEIGHT_OPTION: &str = "placement-height";

impl TuiState {
    pub fn default_with_registry(registry: &PresetRegistry) -> Result<Self> {
        let mut state = Self {
            scene: Scene {
                frame_rate: 30,
                color: true,
                instances: Vec::new(),
            },
            selected_instance: 0,
            selected_option: 0,
            option_names: Vec::new(),
            option_kinds: Vec::new(),
            custom_placements: Vec::new(),
            last_exported_scene: RefCell::new(None),
        };
        state.add_instance("galaxy", registry)?;
        Ok(state)
    }

    pub fn load_startup(registry: &PresetRegistry) -> Result<Self> {
        match Scene::load_default_config_if_available()? {
            Some(scene) => Self::from_scene(scene, registry),
            None => Self::default_with_registry(registry),
        }
    }

    fn from_scene(scene: Scene, registry: &PresetRegistry) -> Result<Self> {
        let mut state = Self {
            last_exported_scene: RefCell::new(None),
            custom_placements: scene
                .instances
                .iter()
                .map(|instance| match &instance.placement {
                    Placement::Custom {
                        x,
                        y,
                        width,
                        height,
                    } => Placement::Custom {
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                    },
                    _ => default_custom_placement(),
                })
                .collect(),
            scene,
            selected_instance: 0,
            selected_option: 0,
            option_names: Vec::new(),
            option_kinds: Vec::new(),
        };
        state.sync_selected_options(registry)?;
        *state.last_exported_scene.borrow_mut() = Some(state.scene.clone());
        Ok(state)
    }

    pub fn export_command(&self) -> String {
        if self.scene.requires_config_export() {
            "ascii-animation run --config ~/.config/ascii-animation/scene.toml".to_string()
        } else {
            self.scene.export_command()
        }
    }

    pub fn export_status(&self) -> Option<String> {
        if !self.scene.requires_config_export() {
            return None;
        }

        if self.last_exported_scene.borrow().as_ref() == Some(&self.scene) {
            None
        } else {
            Some(
                "config export is stale until you press s to save ~/.config/ascii-animation/scene.toml"
                    .to_string(),
            )
        }
    }

    pub fn save_default_scene(&mut self) -> Result<()> {
        self.scene.save_to_path(&Scene::default_config_path())?;
        *self.last_exported_scene.borrow_mut() = Some(self.scene.clone());
        Ok(())
    }

    pub fn preview_text(
        &self,
        registry: &PresetRegistry,
        elapsed_secs: f64,
        width: u16,
        height: u16,
    ) -> Text<'static> {
        render_scene_frame(&self.scene, registry, 0, elapsed_secs, width, height)
            .map(|buffer| frame_to_text(&buffer, self.scene.color))
            .unwrap_or_else(|err| Text::from(err.to_string()))
    }

    pub fn selected_instance(&self) -> &AnimationInstance {
        &self.scene.instances[self.selected_instance]
    }

    pub fn add_instance(&mut self, preset: &str, registry: &PresetRegistry) -> Result<()> {
        let descriptor = registry.get(preset)?;
        let id = next_instance_id(&self.scene.instances, preset, None);
        self.scene.instances.push(AnimationInstance {
            id,
            preset: preset.to_string(),
            options: descriptor.defaults(),
            placement: Placement::Center,
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        });
        self.custom_placements.push(default_custom_placement());
        self.selected_instance = self.scene.instances.len() - 1;
        self.sync_selected_options(registry)
    }

    pub fn remove_selected_instance(&mut self, registry: &PresetRegistry) -> Result<()> {
        if self.scene.instances.len() <= 1 {
            return Ok(());
        }

        self.scene.instances.remove(self.selected_instance);
        self.custom_placements.remove(self.selected_instance);
        self.selected_instance = self.selected_instance.min(self.scene.instances.len() - 1);
        self.sync_selected_options(registry)
    }

    pub fn cycle_selected_instance(
        &mut self,
        delta: i32,
        registry: &PresetRegistry,
    ) -> Result<()> {
        if self.scene.instances.is_empty() {
            return Ok(());
        }

        let len = self.scene.instances.len() as i32;
        self.selected_instance = (self.selected_instance as i32 + delta).rem_euclid(len) as usize;
        self.sync_selected_options(registry)
    }

    pub fn cycle_selected_preset(
        &mut self,
        registry: &PresetRegistry,
        delta: i32,
    ) -> Result<()> {
        let preset_names: Vec<_> = registry.names().collect();
        if preset_names.is_empty() {
            return Ok(());
        }

        let current_preset = self.selected_instance().preset.clone();
        let current_index = preset_names
            .iter()
            .position(|name| *name == current_preset)
            .unwrap_or(0) as i32;
        let next_preset =
            preset_names[(current_index + delta).rem_euclid(preset_names.len() as i32) as usize];
        let next_id = next_instance_id(&self.scene.instances, next_preset, Some(self.selected_instance));
        let descriptor = registry.get(next_preset)?;
        let instance = self.selected_instance_mut();
        instance.id = next_id;
        instance.preset = next_preset.to_string();
        instance.options = descriptor.defaults();
        self.sync_selected_options(registry)
    }

    pub fn set_selected_placement(
        &mut self,
        placement: Placement,
        registry: &PresetRegistry,
    ) -> Result<()> {
        if let Placement::Custom { .. } = &placement {
            self.custom_placements[self.selected_instance] = placement.clone();
        }
        self.selected_instance_mut().placement = placement;
        self.sync_selected_options(registry)
    }

    pub fn cycle_selected_placement(
        &mut self,
        delta: i32,
        registry: &PresetRegistry,
    ) -> Result<()> {
        let current = match &self.selected_instance().placement {
            Placement::Center => 0,
            Placement::Top => 1,
            Placement::Bottom => 2,
            Placement::Left => 3,
            Placement::Right => 4,
            Placement::Fill => 5,
            Placement::Custom { .. } => 6,
        };
        if let Placement::Custom { .. } = &self.selected_instance().placement {
            self.custom_placements[self.selected_instance] = self.selected_instance().placement.clone();
        }
        let placements = [
            Placement::Center,
            Placement::Top,
            Placement::Bottom,
            Placement::Left,
            Placement::Right,
            Placement::Fill,
            self.custom_placements[self.selected_instance].clone(),
        ];
        let next = (current + delta).rem_euclid(placements.len() as i32) as usize;
        self.selected_instance_mut().placement = placements[next].clone();
        self.sync_selected_options(registry)
    }

    pub fn cycle_selected_layer(&mut self, delta: i32) {
        let layers = [Layer::Background, Layer::Normal, Layer::Foreground];
        let current = match self.selected_instance().layer {
            Layer::Background => 0,
            Layer::Normal => 1,
            Layer::Foreground => 2,
        };
        let next = (current + delta).rem_euclid(layers.len() as i32) as usize;
        self.selected_instance_mut().layer = layers[next];
    }

    pub fn adjust_selected_z_index(&mut self, delta: i32) {
        self.selected_instance_mut().z_index += delta;
    }

    pub fn select_option_by_name(&mut self, name: &str) -> Result<()> {
        self.selected_option = self
            .option_names
            .iter()
            .position(|option| option == name)
            .ok_or_else(|| AsciiAnimError::UnknownOption {
                preset: self.selected_instance().preset.clone(),
                option: name.to_string(),
            })?;
        Ok(())
    }

    pub fn next_option(&mut self) {
        if self.option_names.is_empty() {
            return;
        }

        self.selected_option = (self.selected_option + 1) % self.option_names.len();
    }

    pub fn previous_option(&mut self) {
        if self.option_names.is_empty() {
            return;
        }

        self.selected_option = if self.selected_option == 0 {
            self.option_names.len() - 1
        } else {
            self.selected_option - 1
        };
    }

    pub fn adjust_selected_option(&mut self, delta: i32) -> Result<()> {
        if self.option_names.is_empty() {
            return Ok(());
        }

        let option_name = self.option_names[self.selected_option].clone();
        let option_kind = self.option_kinds[self.selected_option].clone();
        {
            let instance = self.selected_instance_mut();
            if adjust_custom_placement(&mut instance.placement, &option_name, delta) {
                let placement = instance.placement.clone();
                self.custom_placements[self.selected_instance] = placement;
                return Ok(());
            }
        }

        let instance = self.selected_instance_mut();
        let current = instance.options.get(&option_name).cloned().ok_or_else(|| {
            AsciiAnimError::UnknownOption {
                preset: instance.preset.clone(),
                option: option_name.clone(),
            }
        })?;
        let next = match (option_kind, current) {
            (OptionKind::Int { min, max, step }, OptionValue::Int(value)) => {
                OptionValue::Int((value + delta as i64 * step).clamp(min, max))
            }
            (OptionKind::Float { min, max }, OptionValue::Float(value)) => {
                OptionValue::Float((value + delta as f64 * 0.01).clamp(min, max))
            }
            (OptionKind::Bool, OptionValue::Bool(value)) => OptionValue::Bool(!value),
            (OptionKind::Choice { choices }, OptionValue::Choice(value)) => {
                rotate_choice(&choices, &value, delta)
            }
            (_, value) => value,
        };
        instance.options.insert(option_name, next);
        Ok(())
    }

    fn selected_instance_mut(&mut self) -> &mut AnimationInstance {
        &mut self.scene.instances[self.selected_instance]
    }

    fn sync_selected_options(&mut self, registry: &PresetRegistry) -> Result<()> {
        let preset = self.selected_instance().preset.clone();
        let descriptor = registry.get(&preset)?;
        let validated = descriptor.validate_options(&self.selected_instance().options)?;
        let (mut option_names, mut option_kinds): (Vec<_>, Vec<_>) = descriptor
            .options()
            .iter()
            .map(|option| (option.name().to_string(), option.kind().clone()))
            .unzip();
        if matches!(self.selected_instance().placement, Placement::Custom { .. }) {
            option_names.extend([
                CUSTOM_X_OPTION.to_string(),
                CUSTOM_Y_OPTION.to_string(),
                CUSTOM_WIDTH_OPTION.to_string(),
                CUSTOM_HEIGHT_OPTION.to_string(),
            ]);
            option_kinds.extend([
                custom_placement_option_kind(CUSTOM_X_OPTION),
                custom_placement_option_kind(CUSTOM_Y_OPTION),
                custom_placement_option_kind(CUSTOM_WIDTH_OPTION),
                custom_placement_option_kind(CUSTOM_HEIGHT_OPTION),
            ]);
        }
        self.selected_instance_mut().options = validated;
        self.option_names = option_names;
        self.option_kinds = option_kinds;
        self.selected_option = self.selected_option.min(self.option_names.len().saturating_sub(1));
        Ok(())
    }
}

pub fn run(registry: &PresetRegistry) -> Result<()> {
    terminal::enable_raw_mode().map_err(terminal_error)?;
    let mut stdout = io::stdout();
    if let Err(err) = execute!(stdout, EnterAlternateScreen) {
        return restore_tui_setup(false).and(Err(terminal_error(err)));
    }
    let entered_alt_screen = true;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(err) => {
            return restore_tui_setup(entered_alt_screen).and(Err(terminal_error(err)));
        }
    };

    let result = (|| {
        let mut state = TuiState::load_startup(registry)?;
        run_loop(&mut terminal, registry, &mut state, Instant::now())
    })();
    let restore_result = restore_tui_terminal(&mut terminal);

    result.and(restore_result)
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    registry: &PresetRegistry,
    state: &mut TuiState,
    started: Instant,
) -> Result<()> {
    loop {
        terminal
            .draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .split(frame.area());

                let preview_width = chunks[0].width.saturating_sub(2).max(1);
                let preview_height = chunks[0].height.saturating_sub(2).max(1);
                let preview = state.preview_text(
                    registry,
                    started.elapsed().as_secs_f64(),
                    preview_width,
                    preview_height,
                );
                frame.render_widget(
                    Paragraph::new(preview)
                        .block(Block::default().title("Preview").borders(Borders::ALL)),
                    chunks[0],
                );

                let instance = state.selected_instance();
                let mut lines = vec![
                    Line::from("Controls: Tab/Shift-Tab instance, ↑/↓ option, ←/→ adjust"),
                    Line::from("a add, d delete, p/P preset, m/M placement, l/L layer, [/ ] z"),
                    Line::from("s save, q quit"),
                    Line::from(""),
                    Line::from(format!(
                        "Selected: {} ({}/{})",
                        instance.id,
                        state.selected_instance + 1,
                        state.scene.instances.len()
                    )),
                    Line::from(format!(
                        "Preset: {}  Placement: {}  Layer: {}  z-index: {}",
                        instance.preset,
                        placement_label(&instance.placement),
                        layer_label(instance.layer),
                        instance.z_index
                    )),
                    Line::from(""),
                    Line::from("Instances:"),
                ];
                for (index, scene_instance) in state.scene.instances.iter().enumerate() {
                    let marker = if index == state.selected_instance { ">" } else { " " };
                    lines.push(Line::from(format!(
                        "{} {} ({})",
                        marker, scene_instance.id, scene_instance.preset
                    )));
                }
                lines.push(Line::from(""));
                lines.push(Line::from("Options:"));
                for (index, name) in state.option_names.iter().enumerate() {
                    let marker = if index == state.selected_option { ">" } else { " " };
                    let value = instance
                        .options
                        .get(name)
                        .cloned()
                        .or_else(|| custom_placement_option_value(&instance.placement, name))
                        .map(|value| value.as_cli_value())
                        .unwrap_or_default();
                    lines.push(Line::from(format!("{} {} = {}", marker, name, value)));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(state.export_command()));
                if let Some(status) = state.export_status() {
                    lines.push(Line::from(status));
                }
                frame.render_widget(
                    Paragraph::new(lines).block(
                        Block::default()
                            .title("Scene / Options / Export")
                            .borders(Borders::ALL),
                    ),
                    chunks[1],
                );
            })
            .map_err(terminal_error)?;

        if event::poll(Duration::from_millis(16)).map_err(terminal_error)? {
            match event::read().map_err(terminal_error)? {
                Event::Key(key) if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc => {
                    return Ok(());
                }
                Event::Key(key) if key.code == KeyCode::Tab => {
                    state.cycle_selected_instance(1, registry)?
                }
                Event::Key(key) if key.code == KeyCode::BackTab => {
                    state.cycle_selected_instance(-1, registry)?
                }
                Event::Key(key) if key.code == KeyCode::Down => state.next_option(),
                Event::Key(key) if key.code == KeyCode::Up => state.previous_option(),
                Event::Key(key) if key.code == KeyCode::Right => state.adjust_selected_option(1)?,
                Event::Key(key) if key.code == KeyCode::Left => state.adjust_selected_option(-1)?,
                Event::Key(key) if key.code == KeyCode::Char('a') => {
                    let preset = state.selected_instance().preset.clone();
                    state.add_instance(&preset, registry)?;
                }
                Event::Key(key) if key.code == KeyCode::Char('d') => {
                    state.remove_selected_instance(registry)?
                }
                Event::Key(key) if key.code == KeyCode::Char('p') => {
                    state.cycle_selected_preset(registry, 1)?
                }
                Event::Key(key) if key.code == KeyCode::Char('P') => {
                    state.cycle_selected_preset(registry, -1)?
                }
                Event::Key(key) if key.code == KeyCode::Char('m') => {
                    state.cycle_selected_placement(1, registry)?
                }
                Event::Key(key) if key.code == KeyCode::Char('M') => {
                    state.cycle_selected_placement(-1, registry)?
                }
                Event::Key(key) if key.code == KeyCode::Char('l') => state.cycle_selected_layer(1),
                Event::Key(key) if key.code == KeyCode::Char('L') => state.cycle_selected_layer(-1),
                Event::Key(key) if key.code == KeyCode::Char(']') => state.adjust_selected_z_index(1),
                Event::Key(key) if key.code == KeyCode::Char('[') => state.adjust_selected_z_index(-1),
                Event::Key(key) if key.code == KeyCode::Char('s') => state.save_default_scene()?,
                _ => {}
            }
        }
    }
}

fn next_instance_id(instances: &[AnimationInstance], preset: &str, skip: Option<usize>) -> String {
    let mut suffix = 1;
    loop {
        let candidate = format!("{}-{}", preset, suffix);
        let exists = instances.iter().enumerate().any(|(index, instance)| {
            skip != Some(index) && instance.id == candidate
        });
        if !exists {
            return candidate;
        }
        suffix += 1;
    }
}

fn placement_label(placement: &Placement) -> &'static str {
    match placement {
        Placement::Center => "center",
        Placement::Top => "top",
        Placement::Bottom => "bottom",
        Placement::Left => "left",
        Placement::Right => "right",
        Placement::Fill => "fill",
        Placement::Custom { .. } => "custom",
    }
}

fn default_custom_placement() -> Placement {
    Placement::Custom {
        x: 0,
        y: 0,
        width: 40,
        height: 12,
    }
}

fn custom_placement_option_kind(name: &str) -> OptionKind {
    match name {
        CUSTOM_X_OPTION | CUSTOM_Y_OPTION => OptionKind::Int {
            min: 0,
            max: i64::from(u16::MAX),
            step: 1,
        },
        CUSTOM_WIDTH_OPTION | CUSTOM_HEIGHT_OPTION => OptionKind::Int {
            min: 1,
            max: i64::from(u16::MAX),
            step: 1,
        },
        _ => unreachable!("unknown custom placement option"),
    }
}

fn custom_placement_option_value(placement: &Placement, name: &str) -> Option<OptionValue> {
    let Placement::Custom {
        x,
        y,
        width,
        height,
    } = placement
    else {
        return None;
    };
    match name {
        CUSTOM_X_OPTION => Some(OptionValue::Int(i64::from(*x))),
        CUSTOM_Y_OPTION => Some(OptionValue::Int(i64::from(*y))),
        CUSTOM_WIDTH_OPTION => Some(OptionValue::Int(i64::from(*width))),
        CUSTOM_HEIGHT_OPTION => Some(OptionValue::Int(i64::from(*height))),
        _ => None,
    }
}

fn adjust_custom_placement(placement: &mut Placement, name: &str, delta: i32) -> bool {
    let Placement::Custom {
        x,
        y,
        width,
        height,
    } = placement
    else {
        return false;
    };
    let clamp = |value: u16, min: u16| -> u16 {
        (i64::from(value) + i64::from(delta))
            .clamp(i64::from(min), i64::from(u16::MAX)) as u16
    };
    match name {
        CUSTOM_X_OPTION => *x = clamp(*x, 0),
        CUSTOM_Y_OPTION => *y = clamp(*y, 0),
        CUSTOM_WIDTH_OPTION => *width = clamp(*width, 1),
        CUSTOM_HEIGHT_OPTION => *height = clamp(*height, 1),
        _ => return false,
    }
    true
}

fn layer_label(layer: Layer) -> &'static str {
    match layer {
        Layer::Background => "background",
        Layer::Normal => "normal",
        Layer::Foreground => "foreground",
    }
}

fn rotate_choice(choices: &[String], current: &str, delta: i32) -> OptionValue {
    let current_index = choices
        .iter()
        .position(|choice| choice == current)
        .unwrap_or(0) as i32;
    let next_index = (current_index + delta).rem_euclid(choices.len() as i32) as usize;
    OptionValue::Choice(choices[next_index].clone())
}

fn frame_to_text(frame: &FrameBuffer, color: bool) -> Text<'static> {
    let mut lines = Vec::with_capacity(frame.height() as usize);
    for y in 0..frame.height() {
        let mut spans = Vec::new();
        let mut current_style = Style::default();
        let mut current_text = String::new();

        for x in 0..frame.width() {
            let cell = frame.get(x, y).expect("coordinates are inside frame");
            let style = if color {
                cell.color
                    .map(|rgb| Style::default().fg(Color::Rgb(rgb.r, rgb.g, rgb.b)))
                    .unwrap_or_default()
            } else {
                Style::default()
            };

            if spans.is_empty() && current_text.is_empty() {
                current_style = style;
            } else if style != current_style {
                spans.push(Span::styled(std::mem::take(&mut current_text), current_style));
                current_style = style;
            }

            current_text.push(cell.ch);
        }

        spans.push(Span::styled(current_text, current_style));
        lines.push(Line::from(spans));
    }

    Text::from(lines)
}

fn restore_tui_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let restore_err = execute!(terminal.backend_mut(), Show, LeaveAlternateScreen).err();
    let disable_err = terminal::disable_raw_mode().err();
    finish_tui_restore(restore_err, disable_err)
}

fn restore_tui_setup(entered_alt_screen: bool) -> Result<()> {
    let restore_err = if entered_alt_screen {
        execute!(io::stdout(), Show, LeaveAlternateScreen).err()
    } else {
        None
    };
    let disable_err = terminal::disable_raw_mode().err();
    finish_tui_restore(restore_err, disable_err)
}

fn finish_tui_restore(
    restore_err: Option<io::Error>,
    disable_err: Option<io::Error>,
) -> Result<()> {
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
