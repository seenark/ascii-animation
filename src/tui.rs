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
}

impl TuiState {
    pub fn default_with_registry(registry: &PresetRegistry) -> Result<Self> {
        let descriptor = registry.get("galaxy")?;
        let (option_names, option_kinds): (Vec<_>, Vec<_>) = descriptor
            .options()
            .iter()
            .map(|option| (option.name().to_string(), option.kind().clone()))
            .unzip();
        Ok(Self {
            scene: Scene {
                frame_rate: 30,
                color: true,
                instances: vec![AnimationInstance {
                    id: "galaxy-1".to_string(),
                    preset: "galaxy".to_string(),
                    options: descriptor.defaults(),
                    placement: Placement::Center,
                    layer: Layer::Normal,
                    z_index: 0,
                    enabled: true,
                }],
            },
            selected_instance: 0,
            selected_option: 0,
            option_names,
            option_kinds,
        })
    }

    pub fn export_command(&self) -> String {
        self.scene.export_command()
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

    pub fn select_option_by_name(&mut self, name: &str) -> Result<()> {
        self.selected_option = self
            .option_names
            .iter()
            .position(|option| option == name)
            .ok_or_else(|| AsciiAnimError::UnknownOption {
                preset: self.scene.instances[self.selected_instance].preset.clone(),
                option: name.to_string(),
            })?;
        Ok(())
    }

    pub fn next_option(&mut self) {
        self.selected_option = (self.selected_option + 1) % self.option_names.len();
    }

    pub fn previous_option(&mut self) {
        self.selected_option = if self.selected_option == 0 {
            self.option_names.len() - 1
        } else {
            self.selected_option - 1
        };
    }

    pub fn adjust_selected_option(&mut self, delta: i32) -> Result<()> {
        let instance = &mut self.scene.instances[self.selected_instance];
        let option_name = self.option_names[self.selected_option].clone();
        let option_kind = self.option_kinds[self.selected_option].clone();
        let current = instance.options.get(&option_name).cloned().ok_or_else(|| {
            AsciiAnimError::UnknownOption {
                preset: instance.preset.clone(),
                option: option_name.clone(),
            }
        })?;
        let next = match (option_kind, current) {
            (OptionKind::Int { min, max }, OptionValue::Int(value)) => {
                OptionValue::Int((value + delta as i64).clamp(min, max))
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
        let mut state = TuiState::default_with_registry(registry)?;
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

                let instance = &state.scene.instances[state.selected_instance];
                let mut lines = vec![
                    Line::from("Controls: ↑/↓ select, ←/→ adjust, s save, q quit"),
                    Line::from(""),
                ];
                for (index, name) in state.option_names.iter().enumerate() {
                    let marker = if index == state.selected_option {
                        ">"
                    } else {
                        " "
                    };
                    let value = instance
                        .options
                        .get(name)
                        .map(OptionValue::as_cli_value)
                        .unwrap_or_default();
                    lines.push(Line::from(format!("{} {} = {}", marker, name, value)));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(state.export_command()));
                frame.render_widget(
                    Paragraph::new(lines).block(
                        Block::default()
                            .title("Options / Export")
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
                Event::Key(key) if key.code == KeyCode::Down => state.next_option(),
                Event::Key(key) if key.code == KeyCode::Up => state.previous_option(),
                Event::Key(key) if key.code == KeyCode::Right => state.adjust_selected_option(1)?,
                Event::Key(key) if key.code == KeyCode::Left => state.adjust_selected_option(-1)?,
                Event::Key(key) if key.code == KeyCode::Char('s') => {
                    state.scene.save_to_path(&Scene::default_config_path())?
                }
                _ => {}
            }
        }
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

fn restore_tui_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let restore_err = execute!(terminal.backend_mut(), Show, LeaveAlternateScreen).err();
    let disable_err = terminal::disable_raw_mode().err();
    finish_tui_restore(restore_err, disable_err)
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
                spans.push(Span::styled(
                    std::mem::take(&mut current_text),
                    current_style,
                ));
                current_style = style;
            }

            current_text.push(cell.ch);
        }

        spans.push(Span::styled(current_text, current_style));
        lines.push(Line::from(spans));
    }

    Text::from(lines)
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
