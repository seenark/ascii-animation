use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::presets::{OptionGroup, OptionKind, OptionValue, PresetRegistry};
use crate::render::buffer::FrameBuffer;
use crate::runtime::{EntryId, SceneSession};
use crate::scene::{AnimationInstance, Layer, Placement, Scene};
use crate::viewport::{editor_layout, split_tui_layout, EditorLayout};
use crate::{AsciiAnimError, Result};

const GRAPHITE: Color = Color::Rgb(24, 26, 29);
const PAPER: Color = Color::Rgb(220, 222, 225);
const MUTED: Color = Color::Rgb(150, 154, 162);
const AMBER: Color = Color::Rgb(239, 183, 79);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFocus { Preview, Scene, Inspector }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorView { Preview, Edit, Scene }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface { None, Browser, Editor, Confirmation, Quit, Export, Help, Recovery }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiAction { Continue, Quit, CopyCommand(String) }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuiLayout { pub options: Rect, pub preview: Rect }
pub fn tui_layout(area: Rect) -> TuiLayout {
    let (options, preview) = split_tui_layout(area);
    TuiLayout { options, preview }
}

#[derive(Clone)]
struct Field { name: String, cli_flag: Option<String>, label: String, kind: OptionKind, group: OptionGroup, help: String, rebuilds: bool }
struct Draft { name: String, value: OptionValue, cursor: usize, error: Option<String> }
struct Browser { search: String, cursor: usize, names: Vec<String>, selected: usize, replace: bool, description_scroll: u16 }
enum Dialog {
    None,
    Browser(Browser),
    Editor(Draft),
    ConfirmDelete,
    ConfirmReplace(String),
    Quit(usize),
    Export { scroll: u16, choice: usize },
    Help(u16),
    Recovery(u16),
}

pub struct TuiState {
    pub scene: Scene,
    session: SceneSession,
    entry_ids: Vec<EntryId>,
    saved_scene: Option<Scene>,
    config_path: PathBuf,
    selected_instance: usize,
    selected_option: usize,
    fields: Vec<Field>,
    option_names: Vec<String>,
    custom_placements: Vec<Placement>,
    focus: PaneFocus,
    return_focus: PaneFocus,
    view: EditorView,
    fullscreen: bool,
    fullscreen_focus: Option<PaneFocus>,
    dialog: Dialog,
    temporary: Option<SceneSession>,
    status: Option<String>,
    copy_status: Option<String>,
    startup_error: Option<String>,
    scene_scroll: usize,
    inspector_scroll: usize,
    terminal_size: (u16, u16),
}

pub fn format_tui_option_value(value: &OptionValue) -> String {
    match value { OptionValue::Float(v) => format!("{v:.2}"), other => other.as_cli_value() }
}

impl TuiState {
    pub fn default_with_registry(registry: &PresetRegistry) -> Result<Self> {
        let scene = Scene { frame_rate: 30, color: true, instances: vec![new_instance("galaxy", &[], registry)?] };
        Self::from_scene(scene, registry)
    }
    pub fn from_scene(scene: Scene, registry: &PresetRegistry) -> Result<Self> {
        Self::new(scene, None, Scene::default_config_path(), registry)
    }
    fn new(scene: Scene, saved_scene: Option<Scene>, config_path: PathBuf, registry: &PresetRegistry) -> Result<Self> {
        let session = SceneSession::new(scene, registry, 0)?;
        let scene = session.scene().clone();
        let entry_ids = session.entry_ids();
        let custom_placements = scene.instances.iter().map(|i| match i.placement {
            Placement::Custom { .. } => i.placement.clone(), _ => default_custom_placement(),
        }).collect();
        let mut state = Self { scene, session, entry_ids, saved_scene, config_path, selected_instance: 0,
            selected_option: 0, fields: Vec::new(), option_names: Vec::new(), custom_placements,
            focus: PaneFocus::Preview, return_focus: PaneFocus::Preview, view: EditorView::Preview,
            fullscreen: false, fullscreen_focus: None, dialog: Dialog::None, temporary: None, status: None, copy_status: None,
            startup_error: None, scene_scroll: 0, inspector_scroll: 0, terminal_size: (120, 38) };
        state.sync_selected_options(registry)?;
        Ok(state)
    }
    pub fn load_startup(registry: &PresetRegistry) -> Result<Self> {
        Self::load_from_path(Scene::default_config_path(), registry)
    }
    pub fn load_from_path(path: impl AsRef<Path>, registry: &PresetRegistry) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let loaded = Scene::load_from_path_raw(&path);
        match loaded {
            Ok(baseline) => {
                match normalize_startup_scene(baseline.clone(), registry)
                    .and_then(|scene| Self::new(scene, Some(baseline), path.clone(), registry)) {
                    Ok(state) => Ok(state),
                    Err(err) => Self::recovery(path, err.to_string(), registry),
                }
            }
            Err(_) if matches!(path.try_exists(), Ok(false)) => {
                let mut state = Self::default_with_registry(registry)?;
                state.config_path = path;
                Ok(state)
            }
            Err(err) => Self::recovery(path, err.to_string(), registry),
        }
    }
    fn recovery(path: PathBuf, error: String, registry: &PresetRegistry) -> Result<Self> {
        let mut state = Self::default_with_registry(registry)?;
        state.config_path = path;
        state.startup_error = Some(error);
        state.open(Dialog::Recovery(0));
        Ok(state)
    }
    pub fn startup_error(&self) -> Option<&str> { self.startup_error.as_deref() }
    pub fn config_path(&self) -> &Path { &self.config_path }
    pub fn is_dirty(&self) -> bool { self.saved_scene.as_ref() != Some(&self.scene) }
    pub fn focus(&self) -> PaneFocus { self.focus }
    pub fn view(&self) -> EditorView { self.view }
    pub fn fullscreen(&self) -> bool { self.fullscreen }
    pub fn is_paused(&self) -> bool { self.session.is_paused() }
    pub fn elapsed_seconds(&self) -> f64 { self.session.elapsed_seconds() }
    pub fn status(&self) -> Option<&str> {
        if let Dialog::Editor(draft) = &self.dialog {
            if let Some(error) = &draft.error { return Some(error); }
        }
        self.status.as_deref()
    }
    pub fn copy_status(&self) -> Option<&str> { self.copy_status.as_deref() }
    pub fn surface(&self) -> Surface {
        match self.dialog { Dialog::None => Surface::None, Dialog::Browser(_) => Surface::Browser,
            Dialog::Editor(_) => Surface::Editor, Dialog::ConfirmDelete | Dialog::ConfirmReplace(_) => Surface::Confirmation,
            Dialog::Quit(_) => Surface::Quit, Dialog::Export { .. } => Surface::Export,
            Dialog::Help(_) => Surface::Help, Dialog::Recovery(_) => Surface::Recovery }
    }
    pub fn draft_text(&self) -> Option<&str> {
        match &self.dialog { Dialog::Editor(Draft { value: OptionValue::Text(value), .. }) => Some(value),
            Dialog::Browser(browser) => Some(&browser.search), _ => None }
    }
    pub fn selected_option_name(&self) -> Option<&str> { self.option_names.get(self.selected_option).map(String::as_str) }
    pub fn selected_instance_index(&self) -> usize { self.selected_instance }
    pub fn selected_instance(&self) -> &AnimationInstance { &self.scene.instances[self.selected_instance] }
    pub fn visible_option_names(&self) -> &[String] { &self.option_names }
    pub fn editing_text(&self) -> bool { matches!(&self.dialog, Dialog::Editor(Draft { value: OptionValue::Text(_), .. })) }
    pub fn selected_option_is_text(&self) -> bool { matches!(self.fields.get(self.selected_option).map(|f| &f.kind), Some(OptionKind::Text { .. })) }
    fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        if self.fullscreen {
            self.fullscreen_focus = Some(self.focus);
            self.focus = PaneFocus::Preview;
        } else if let Some(focus) = self.fullscreen_focus.take() {
            self.focus = focus;
        }
    }
    pub fn resize(&mut self, width: u16, height: u16) { self.terminal_size = (width, height); }
    pub fn advance(&mut self, delta: Duration) {
        self.session.advance(delta);
        if let Some(temporary) = &mut self.temporary {
            temporary.set_paused(self.session.is_paused());
            temporary.advance(delta);
        }
    }
    pub fn preview_text(&mut self, width: u16, height: u16) -> Text<'static> {
        // Draw the live session even behind a draft so stateful entries continue advancing.
        let live = self.session.draw(width, height);
        match live {
            Err(err) => { self.status = Some(err.to_string()); Text::from(err.to_string()) }
            Ok(frame) => {
                if let Some(temporary) = &mut self.temporary {
                    match temporary.draw(width, height) {
                        Ok(frame) => frame_to_text(frame, self.scene.color),
                        Err(err) => { self.status = Some(err.to_string()); Text::from(err.to_string()) }
                    }
                } else { frame_to_text(frame, self.scene.color) }
            }
        }
    }
    pub fn export_command(&self) -> String {
        if self.scene.requires_config_export() {
            if self.config_path == Scene::default_config_path() { self.scene.export_command() }
            else { format!("ascii-animation run --config {}", shell_quote(&self.config_path.to_string_lossy())) }
        } else { self.scene.export_command() }
    }
    pub fn export_status(&self) -> Option<String> {
        (self.scene.requires_config_export() && self.is_dirty()).then(|| "Save current Scene before copying its config command".into())
    }
    pub fn set_copy_status(&mut self, result: std::result::Result<(), String>) {
        self.copy_status = Some(match result { Ok(()) => "Copied command to clipboard".into(), Err(message) => format!("Copy failed: {message}; command remains available") });
    }
    pub fn save_default_scene(&mut self) -> Result<()> {
        match self.scene.save_to_path(&self.config_path) {
            Ok(()) => { self.saved_scene = Some(self.scene.clone()); self.status = Some("Scene saved".into()); Ok(()) }
            Err(err) => { self.status = Some(format!("Save failed: {err}")); Err(err) }
        }
    }
    fn open(&mut self, dialog: Dialog) { self.return_focus = self.focus; self.dialog = dialog; self.copy_status = None; }
    fn close(&mut self) { self.dialog = Dialog::None; self.temporary = None; self.focus = self.return_focus; }
    fn commit(&mut self, scene: Scene, ids: Vec<Option<EntryId>>, registry: &PresetRegistry) -> Result<()> {
        let restarted = self.session.apply_scene(scene, &ids)?;
        self.scene = self.session.scene().clone();
        self.entry_ids = self.session.entry_ids();
        if !restarted.is_empty() { self.status = Some(format!("Restarted {} animation instance(s) after state change", restarted.len())); }
        self.sync_selected_options(registry)
    }
    fn identities(&self) -> Vec<Option<EntryId>> { self.entry_ids.iter().copied().map(Some).collect() }
    pub fn add_instance(&mut self, preset: &str, registry: &PresetRegistry) -> Result<()> {
        let mut scene = self.scene.clone();
        scene.instances.push(new_instance(preset, &scene.instances, registry)?);
        let mut ids = self.identities(); ids.push(None);
        let selected = self.selected_instance;
        self.selected_instance = scene.instances.len() - 1;
        if let Err(err) = self.commit(scene, ids, registry) { self.selected_instance = selected; return Err(err); }
        self.custom_placements.push(default_custom_placement());
        self.selected_option = 0;
        self.focus = PaneFocus::Inspector;
        self.view = EditorView::Edit;
        self.fullscreen = false;
        self.fullscreen_focus = None;
        Ok(())
    }
    pub fn remove_selected_instance(&mut self, registry: &PresetRegistry) -> Result<()> {
        if self.scene.instances.len() == 1 { self.status = Some("Cannot delete final animation instance; add another first".into()); return Ok(()); }
        let mut scene = self.scene.clone(); let mut ids = self.identities();
        scene.instances.remove(self.selected_instance); ids.remove(self.selected_instance);
        let old = self.selected_instance;
        self.selected_instance = old.min(scene.instances.len() - 1);
        if let Err(err) = self.commit(scene, ids, registry) { self.selected_instance = old; return Err(err); }
        self.custom_placements.remove(old);
        Ok(())
    }
    pub fn cycle_selected_instance(&mut self, delta: i32, registry: &PresetRegistry) -> Result<()> {
        self.selected_instance = (self.selected_instance as i32 + delta).rem_euclid(self.scene.instances.len() as i32) as usize;
        self.selected_option = 0;
        self.sync_selected_options(registry)
    }
    pub fn move_selected_instance(&mut self, delta: i32, registry: &PresetRegistry) -> Result<()> {
        let next = (self.selected_instance as i32 + delta).clamp(0, self.scene.instances.len() as i32 - 1) as usize;
        let mut scene = self.scene.clone(); let mut ids = self.identities();
        scene.instances.swap(self.selected_instance, next); ids.swap(self.selected_instance, next);
        self.commit(scene, ids, registry)?;
        self.custom_placements.swap(self.selected_instance, next);
        self.selected_instance = next;
        self.sync_selected_options(registry)
    }
    fn replace_selected(&mut self, preset: &str, registry: &PresetRegistry) -> Result<()> {
        let mut scene = self.scene.clone();
        let descriptor = registry.get(preset)?;
        let instance = &mut scene.instances[self.selected_instance];
        instance.preset = preset.into(); instance.options = descriptor.defaults();
        self.commit(scene, self.identities(), registry)
    }
    pub fn set_selected_placement(&mut self, placement: Placement, registry: &PresetRegistry) -> Result<()> {
        let mut scene = self.scene.clone(); scene.instances[self.selected_instance].placement = placement.clone();
        self.commit(scene, self.identities(), registry)?;
        if matches!(placement, Placement::Custom { .. }) { self.custom_placements[self.selected_instance] = placement; }
        Ok(())
    }
    pub fn select_option_by_name(&mut self, name: &str) -> Result<()> {
        self.selected_option = self.option_names.iter().position(|n| n == name).ok_or_else(|| AsciiAnimError::UnknownOption { preset: self.selected_instance().preset.clone(), option: name.into() })?;
        self.focus = PaneFocus::Inspector; self.view = EditorView::Edit;
        Ok(())
    }
    pub fn next_option(&mut self) { if !self.fields.is_empty() { self.selected_option = (self.selected_option + 1) % self.fields.len(); } }
    pub fn previous_option(&mut self) { if !self.fields.is_empty() { self.selected_option = (self.selected_option + self.fields.len() - 1) % self.fields.len(); } }
    pub fn adjust_selected_option(&mut self, delta: i32, registry: &PresetRegistry) -> Result<()> {
        let Some(field) = self.fields.get(self.selected_option).cloned() else { return Ok(()); };
        let value = self.field_value(&field.name).expect("visible field has value");
        let next = adjusted(&field.kind, value, delta);
        let mut scene = self.scene.clone();
        set_field(&mut scene.instances[self.selected_instance], &field.name, next, &self.custom_placements[self.selected_instance]);
        self.commit(scene, self.identities(), registry)?;
        if matches!(self.selected_instance().placement, Placement::Custom { .. }) {
            self.custom_placements[self.selected_instance] = self.selected_instance().placement.clone();
        }
        Ok(())
    }
    fn field_value(&self, name: &str) -> Option<OptionValue> {
        let instance = self.selected_instance();
        match name {
            "enabled" => Some(OptionValue::Bool(instance.enabled)),
            "placement" => Some(OptionValue::Choice(placement_label(&instance.placement).into())),
            "layer" => Some(OptionValue::Choice(layer_label(instance.layer).into())),
            "z-index" => Some(OptionValue::Int(instance.z_index as i64)),
            "placement-x" | "placement-y" | "placement-width" | "placement-height" => custom_value(&instance.placement, name),
            _ => instance.options.get(name).cloned(),
        }
    }
    fn sync_selected_options(&mut self, registry: &PresetRegistry) -> Result<()> {
        let selected_name = self.selected_option_name().map(str::to_string);
        let instance = self.selected_instance();
        let descriptor = registry.get(&instance.preset)?;
        let mut fields: Vec<_> = descriptor.visible_options(&instance.options).into_iter().map(|o| Field {
            name: o.name().into(), cli_flag: Some(format!("--{}", o.name())), label: o.label().into(), kind: o.kind().clone(), group: o.group(), help: o.help().into(), rebuilds: o.rebuilds_state(),
        }).collect();
        fields.sort_by_key(|f| group_index(f.group));
        fields.push(layout_field("enabled", "Enabled", OptionKind::Bool, "Disabled instances keep progressing without drawing."));
        fields.push(layout_field("placement", "Placement", choices(&["center", "top", "bottom", "left", "right", "fill", "custom"]), "Fill covers Canvas; Center is the creation default."));
        fields.push(layout_field("layer", "Layer", choices(&["background", "normal", "foreground"]), "Full-field backgrounds usually use Fill and Background."));
        fields.push(layout_field("z-index", "Z-index", OptionKind::Int { min: i32::MIN as i64, max: i32::MAX as i64, step: 1 }, "Higher Z-index appears in front within its Layer."));
        if matches!(instance.placement, Placement::Custom { .. }) {
            for (name, label, min) in [("placement-x", "Region X", 0), ("placement-y", "Region Y", 0), ("placement-width", "Region width", 1), ("placement-height", "Region height", 1)] {
                fields.push(layout_field(name, label, OptionKind::Int { min, max: u16::MAX as i64, step: 1 }, "Custom region in Canvas cells."));
            }
        }
        self.fields = fields;
        self.option_names = self.fields.iter().map(|f| f.name.clone()).collect();
        self.selected_option = selected_name.and_then(|n| self.option_names.iter().position(|v| v == &n)).unwrap_or(self.selected_option.min(self.fields.len().saturating_sub(1)));
        Ok(())
    }
    fn begin_edit(&mut self) {
        if let Some(field) = self.fields.get(self.selected_option) {
            if matches!(field.kind, OptionKind::Text { .. } | OptionKind::Choice { .. }) {
                let value = self.field_value(&field.name).unwrap();
                let cursor = value.as_cli_value().len();
                self.open(Dialog::Editor(Draft { name: field.name.clone(), value, cursor, error: None }));
            }
        }
    }
    fn refresh_draft_preview(&mut self, registry: &PresetRegistry) {
        let Dialog::Editor(draft) = &self.dialog else { return; };
        let mut scene = self.scene.clone();
        set_field(&mut scene.instances[self.selected_instance], &draft.name, draft.value.clone(), &self.custom_placements[self.selected_instance]);
        match SceneSession::new(scene, registry, 0) {
            Ok(mut temporary) => { temporary.set_paused(self.session.is_paused()); self.temporary = Some(temporary); if let Dialog::Editor(draft) = &mut self.dialog { draft.error = None; } }
            Err(err) => { if let Dialog::Editor(draft) = &mut self.dialog { draft.error = Some(field_error_message(&err)); } }
        }
    }
    fn open_browser(&mut self, replace: bool, registry: &PresetRegistry) -> Result<()> {
        self.open(Dialog::Browser(Browser { search: String::new(), cursor: 0, names: registry.names().map(str::to_string).collect(), selected: 0, replace, description_scroll: 0 }));
        self.refresh_browser(registry)
    }
    fn refresh_browser(&mut self, registry: &PresetRegistry) -> Result<()> {
        let Dialog::Browser(browser) = &mut self.dialog else { return Ok(()); };
        let search = browser.search.to_ascii_lowercase();
        browser.names = registry.names().filter(|name| {
            let d = registry.get(name).expect("registry name exists");
            name.to_ascii_lowercase().contains(&search) || d.label().to_ascii_lowercase().contains(&search)
        }).map(str::to_string).collect();
        browser.selected = browser.selected.min(browser.names.len().saturating_sub(1));
        if let Some(name) = browser.names.get(browser.selected) {
            if self.temporary.as_ref().is_some_and(|session| session.scene().instances[0].preset == *name) { return Ok(()); }
            let scene = Scene { frame_rate: self.scene.frame_rate, color: self.scene.color, instances: vec![new_instance(name, &[], registry)?] };
            let mut session = SceneSession::new(scene, registry, 0)?;
            session.set_paused(self.session.is_paused());
            self.temporary = Some(session);
        } else { self.temporary = None; }
        Ok(())
    }
}

pub fn handle_tui_key(state: &mut TuiState, key: KeyEvent, registry: &PresetRegistry) -> Result<TuiAction> {
    if key.kind == KeyEventKind::Release { return Ok(TuiAction::Continue); }
    if editor_layout(state.terminal_size.0, state.terminal_size.1) == EditorLayout::Tiny {
        if key.code == KeyCode::Esc { state.close(); }
        if key.code == KeyCode::Char('q')
            && ((!state.is_dirty() && matches!(state.dialog, Dialog::None)) || matches!(state.dialog, Dialog::Recovery(_))) {
            return Ok(TuiAction::Quit);
        }
        return Ok(TuiAction::Continue);
    }
    let dialog = std::mem::replace(&mut state.dialog, Dialog::None);
    match dialog {
        Dialog::Editor(mut draft) => {
            if key.code == KeyCode::Esc { state.close(); return Ok(TuiAction::Continue); }
            if key.code == KeyCode::Enter {
                let mut scene = state.scene.clone();
                set_field(&mut scene.instances[state.selected_instance], &draft.name, draft.value.clone(), &state.custom_placements[state.selected_instance]);
                match state.commit(scene, state.identities(), registry) {
                    Ok(()) => state.close(),
                    Err(err) => { draft.error = Some(field_error_message(&err)); state.dialog = Dialog::Editor(draft); }
                }
                return Ok(TuiAction::Continue);
            }
            let field = state.fields.iter().find(|f| f.name == draft.name).expect("draft field remains selected");
            let changed = match (&mut draft.value, &field.kind) {
                (OptionValue::Text(text), OptionKind::Text { max_len }) => edit_text(text, &mut draft.cursor, key, Some(*max_len)),
                (value @ OptionValue::Choice(_), kind) => match key.code {
                    KeyCode::Left | KeyCode::Up => { *value = adjusted(kind, value.clone(), -1); true },
                    KeyCode::Right | KeyCode::Down => { *value = adjusted(kind, value.clone(), 1); true },
                    KeyCode::Home => { if let OptionKind::Choice { choices } = kind { *value = OptionValue::Choice(choices[0].clone()); } true },
                    KeyCode::End => { if let OptionKind::Choice { choices } = kind { *value = OptionValue::Choice(choices.last().unwrap().clone()); } true },
                    _ => false,
                },
                _ => false,
            };
            state.dialog = Dialog::Editor(draft);
            if changed { state.refresh_draft_preview(registry); }
        }
        Dialog::Browser(mut browser) => {
            match key.code {
                KeyCode::Esc => { state.close(); return Ok(TuiAction::Continue); }
                KeyCode::Enter => {
                    if let Some(name) = browser.names.get(browser.selected).cloned() {
                        if browser.replace { state.dialog = Dialog::ConfirmReplace(name); state.temporary = None; }
                        else { state.add_instance(&name, registry)?; state.close(); state.focus = PaneFocus::Inspector; state.view = EditorView::Edit; }
                        return Ok(TuiAction::Continue);
                    }
                }
                KeyCode::Down if !browser.names.is_empty() => { browser.selected = (browser.selected + 1) % browser.names.len(); browser.description_scroll = 0; }
                KeyCode::Up if !browser.names.is_empty() => { browser.selected = (browser.selected + browser.names.len() - 1) % browser.names.len(); browser.description_scroll = 0; }
                KeyCode::PageDown => browser.description_scroll = browser.description_scroll.saturating_add(1),
                KeyCode::PageUp => browser.description_scroll = browser.description_scroll.saturating_sub(1),
                _ => { if edit_text(&mut browser.search, &mut browser.cursor, key, None) { browser.selected = 0; browser.description_scroll = 0; } }
            }
            state.dialog = Dialog::Browser(browser); state.refresh_browser(registry)?;
        }
        Dialog::ConfirmDelete => {
            match key.code {
                KeyCode::Enter | KeyCode::Char('y') => { state.remove_selected_instance(registry)?; state.close(); }
                KeyCode::Esc | KeyCode::Char('n') => state.close(),
                _ => state.dialog = Dialog::ConfirmDelete,
            }
        }
        Dialog::ConfirmReplace(name) => {
            match key.code {
                KeyCode::Enter | KeyCode::Char('y') => { state.replace_selected(&name, registry)?; state.close(); }
                KeyCode::Esc | KeyCode::Char('n') => state.close(),
                _ => state.dialog = Dialog::ConfirmReplace(name),
            }
        }
        Dialog::Quit(mut choice) => {
            match key.code {
                KeyCode::Esc => { state.close(); return Ok(TuiAction::Continue); }
                KeyCode::Left | KeyCode::Up => choice = (choice + 2) % 3,
                KeyCode::Right | KeyCode::Down | KeyCode::Tab => choice = (choice + 1) % 3,
                KeyCode::Char('s') => choice = 0,
                KeyCode::Char('d') => return Ok(TuiAction::Quit),
                KeyCode::Char('c') => { state.close(); return Ok(TuiAction::Continue); }
                _ => {},
            }
            if key.code == KeyCode::Enter || key.code == KeyCode::Char('s') {
                match choice {
                    0 => if state.save_default_scene().is_ok() { return Ok(TuiAction::Quit); },
                    1 => return Ok(TuiAction::Quit),
                    _ => { state.close(); return Ok(TuiAction::Continue); }
                }
            }
            state.dialog = Dialog::Quit(choice);
        }
        Dialog::Export { mut scroll, mut choice } => {
            if key.code == KeyCode::Esc { state.close(); return Ok(TuiAction::Continue); }
            match key.code {
                KeyCode::Down => scroll = scroll.saturating_add(1), KeyCode::Up => scroll = scroll.saturating_sub(1),
                KeyCode::PageDown => scroll = scroll.saturating_add(8), KeyCode::PageUp => scroll = scroll.saturating_sub(8),
                KeyCode::Home => scroll = 0,
                KeyCode::Tab | KeyCode::Left | KeyCode::Right => choice = 1 - choice,
                KeyCode::Enter if choice == 1 => { state.close(); return Ok(TuiAction::Continue); }
                KeyCode::Enter | KeyCode::Char('c') => {
                    if state.scene.requires_config_export() && state.is_dirty() && state.save_default_scene().is_err() {
                        state.dialog = Dialog::Export { scroll, choice }; return Ok(TuiAction::Continue);
                    }
                    state.dialog = Dialog::Export { scroll, choice };
                    return Ok(TuiAction::CopyCommand(state.export_command()));
                }
                KeyCode::Char('s') => { let _ = state.save_default_scene(); }
                _ => {},
            }
            state.dialog = Dialog::Export { scroll, choice };
        }
        Dialog::Help(mut scroll) => {
            match key.code { KeyCode::Esc | KeyCode::Char('?') => state.close(),
                KeyCode::Down => { scroll = scroll.saturating_add(1); state.dialog = Dialog::Help(scroll); },
                KeyCode::Up => { scroll = scroll.saturating_sub(1); state.dialog = Dialog::Help(scroll); },
                _ => state.dialog = Dialog::Help(scroll) }
        }
        Dialog::Recovery(mut scroll) => {
            match key.code {
                KeyCode::Char('r') => { *state = TuiState::load_from_path(&state.config_path, registry)?; }
                KeyCode::Enter | KeyCode::Char('d') => { state.close(); state.status = Some("Opened unsaved default; original file remains unchanged".into()); }
                KeyCode::Char('q') => return Ok(TuiAction::Quit),
                KeyCode::Down => { scroll = scroll.saturating_add(1); state.dialog = Dialog::Recovery(scroll); }
                KeyCode::Up => { scroll = scroll.saturating_sub(1); state.dialog = Dialog::Recovery(scroll); }
                KeyCode::PageDown => { scroll = scroll.saturating_add(6); state.dialog = Dialog::Recovery(scroll); }
                KeyCode::PageUp => { scroll = scroll.saturating_sub(6); state.dialog = Dialog::Recovery(scroll); }
                KeyCode::Home => state.dialog = Dialog::Recovery(0),
                _ => state.dialog = Dialog::Recovery(scroll),
            }
        }
        Dialog::None => {
            match key.code {
                KeyCode::Char('q') => if state.is_dirty() { state.open(Dialog::Quit(2)); } else { return Ok(TuiAction::Quit); },
                KeyCode::Char(' ') => state.session.set_paused(!state.session.is_paused()),
                KeyCode::Char('f') => state.toggle_fullscreen(),
                KeyCode::Char('a') => state.open_browser(false, registry)?,
                KeyCode::Char('r') if state.focus == PaneFocus::Scene => state.open_browser(true, registry)?,
                KeyCode::Char('s') => { let _ = state.save_default_scene(); },
                KeyCode::Char('c') => state.open(Dialog::Export { scroll: 0, choice: 0 }),
                KeyCode::Char('?') => state.open(Dialog::Help(0)),
                KeyCode::Esc => if state.fullscreen { state.toggle_fullscreen(); },
                KeyCode::Tab | KeyCode::BackTab if state.fullscreen => {},
                KeyCode::Tab | KeyCode::BackTab => {
                    state.focus = match (state.focus, key.code) {
                        (PaneFocus::Preview, KeyCode::Tab) | (PaneFocus::Inspector, KeyCode::BackTab) => PaneFocus::Scene,
                        (PaneFocus::Scene, KeyCode::Tab) | (PaneFocus::Preview, KeyCode::BackTab) => PaneFocus::Inspector,
                        _ => PaneFocus::Preview,
                    };
                    state.view = match state.focus { PaneFocus::Preview => EditorView::Preview, PaneFocus::Scene => EditorView::Scene, PaneFocus::Inspector => EditorView::Edit };
                }
                KeyCode::Down if state.focus == PaneFocus::Scene => state.cycle_selected_instance(1, registry)?,
                KeyCode::Up if state.focus == PaneFocus::Scene => state.cycle_selected_instance(-1, registry)?,
                KeyCode::Down if state.focus == PaneFocus::Inspector => state.next_option(),
                KeyCode::Up if state.focus == PaneFocus::Inspector => state.previous_option(),
                KeyCode::Home if state.focus == PaneFocus::Inspector => state.selected_option = 0,
                KeyCode::End if state.focus == PaneFocus::Inspector => state.selected_option = state.fields.len().saturating_sub(1),
                KeyCode::Enter if state.focus == PaneFocus::Scene => { state.focus = PaneFocus::Inspector; state.view = EditorView::Edit; },
                KeyCode::Enter if state.focus == PaneFocus::Inspector => {
                    if state.selected_option_is_text() || matches!(state.fields[state.selected_option].kind, OptionKind::Choice { .. }) { state.begin_edit(); }
                    else { state.adjust_selected_option(1, registry)?; }
                }
                KeyCode::Left | KeyCode::Right if state.focus == PaneFocus::Inspector => {
                    let direction = if key.code == KeyCode::Left { -1 } else { 1 };
                    let field = &state.fields[state.selected_option];
                    let fast = key.modifiers.contains(KeyModifiers::SHIFT) && matches!(field.kind, OptionKind::Int { .. } | OptionKind::Float { .. });
                    state.adjust_selected_option(direction * if fast { 10 } else { 1 }, registry)?;
                }
                KeyCode::Delete | KeyCode::Char('d') if state.focus == PaneFocus::Scene => {
                    if state.scene.instances.len() == 1 { state.status = Some("Cannot delete final animation instance; add another first".into()); }
                    else { state.open(Dialog::ConfirmDelete); }
                }
                KeyCode::Char('[') if state.focus == PaneFocus::Scene => state.move_selected_instance(-1, registry)?,
                KeyCode::Char(']') if state.focus == PaneFocus::Scene => state.move_selected_instance(1, registry)?,
                _ => {},
            }
        }
    }
    Ok(TuiAction::Continue)
}

fn field_error_message(error: &AsciiAnimError) -> String {
    match error {
        AsciiAnimError::AnimationInstance { source, .. } => field_error_message(source),
        AsciiAnimError::InvalidOptionType { expected, .. } => format!("Expected {expected}"),
        AsciiAnimError::TextTooLong { max, .. } => format!("Use at most {max} characters"),
        AsciiAnimError::OptionOutOfRange { min, max, .. } => format!("Expected {min}..{max}"),
        AsciiAnimError::InvalidChoice { .. } => "Choose an available value".into(),
        _ => error.to_string(),
    }
}

fn edit_text(text: &mut String, cursor: &mut usize, key: KeyEvent, max_len: Option<usize>) -> bool {
    match key.code {
        KeyCode::Left => *cursor = text[..*cursor].char_indices().last().map(|(index, _)| index).unwrap_or(0),
        KeyCode::Right => *cursor += text[*cursor..].chars().next().map(char::len_utf8).unwrap_or(0),
        KeyCode::Home => *cursor = 0, KeyCode::End => *cursor = text.len(),
        KeyCode::Backspace if *cursor > 0 => {
            *cursor = text[..*cursor].char_indices().last().map(|(index, _)| index).unwrap_or(0);
            text.remove(*cursor); return true;
        }
        KeyCode::Delete if *cursor < text.len() => { text.remove(*cursor); return true; }
        KeyCode::Char(ch) if (key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT) && !ch.is_control() => {
            if max_len.map_or(true, |max| text.chars().count() < max) {
                text.insert(*cursor, ch); *cursor += ch.len_utf8(); return true;
            }
        }
        _ => {},
    }
    false
}
fn choices(values: &[&str]) -> OptionKind { OptionKind::Choice { choices: values.iter().map(|s| s.to_string()).collect() } }
fn layout_field(name: &str, label: &str, kind: OptionKind, help: &str) -> Field {
    Field { name: name.into(), cli_flag: None, label: label.into(), kind, group: OptionGroup::Layout, help: help.into(), rebuilds: false }
}
fn group_index(group: OptionGroup) -> u8 { match group { OptionGroup::Basics => 0, OptionGroup::Motion => 1, OptionGroup::Style => 2, OptionGroup::Layout => 3 } }
fn group_label(group: OptionGroup) -> &'static str { match group { OptionGroup::Basics => "Basics", OptionGroup::Motion => "Motion", OptionGroup::Style => "Style", OptionGroup::Layout => "Layout (advanced)" } }
fn adjusted(kind: &OptionKind, value: OptionValue, delta: i32) -> OptionValue {
    match (kind, value) {
        (OptionKind::Int { min, max, step }, OptionValue::Int(v)) => OptionValue::Int(v.saturating_add(i64::from(delta) * step).clamp(*min, *max)),
        (OptionKind::Float { min, max }, OptionValue::Float(v)) => OptionValue::Float((v + f64::from(delta) * 0.01).clamp(*min, *max)),
        (OptionKind::Bool, OptionValue::Bool(v)) => OptionValue::Bool(!v),
        (OptionKind::Choice { choices }, OptionValue::Choice(v)) if !choices.is_empty() => {
            let index = choices.iter().position(|s| s == &v).unwrap_or(0) as i32;
            OptionValue::Choice(choices[(index + delta).rem_euclid(choices.len() as i32) as usize].clone())
        }
        (_, value) => value,
    }
}
fn set_field(instance: &mut AnimationInstance, name: &str, value: OptionValue, custom: &Placement) {
    match (name, value) {
        ("enabled", OptionValue::Bool(v)) => instance.enabled = v,
        ("z-index", OptionValue::Int(v)) => instance.z_index = v as i32,
        ("layer", OptionValue::Choice(v)) => instance.layer = match v.as_str() { "background" => Layer::Background, "foreground" => Layer::Foreground, _ => Layer::Normal },
        ("placement", OptionValue::Choice(v)) => instance.placement = match v.as_str() {
            "top" => Placement::Top, "bottom" => Placement::Bottom, "left" => Placement::Left,
            "right" => Placement::Right, "fill" => Placement::Fill, "custom" => custom.clone(), _ => Placement::Center,
        },
        (name @ ("placement-x" | "placement-y" | "placement-width" | "placement-height"), OptionValue::Int(v)) => {
            if let Placement::Custom { x, y, width, height } = &mut instance.placement {
                match name { "placement-x" => *x = v as u16, "placement-y" => *y = v as u16, "placement-width" => *width = v as u16, _ => *height = v as u16 }
            }
        }
        (name, value) => { instance.options.insert(name.into(), value); }
    }
}
fn custom_value(placement: &Placement, name: &str) -> Option<OptionValue> {
    let Placement::Custom { x, y, width, height } = placement else { return None; };
    Some(OptionValue::Int(i64::from(match name { "placement-x" => *x, "placement-y" => *y, "placement-width" => *width, _ => *height })))
}
fn default_custom_placement() -> Placement { Placement::Custom { x: 0, y: 0, width: 40, height: 12 } }
fn placement_label(placement: &Placement) -> &'static str {
    match placement { Placement::Center => "center", Placement::Top => "top", Placement::Bottom => "bottom", Placement::Left => "left", Placement::Right => "right", Placement::Fill => "fill", Placement::Custom { .. } => "custom" }
}
fn layer_label(layer: Layer) -> &'static str { match layer { Layer::Background => "background", Layer::Normal => "normal", Layer::Foreground => "foreground" } }
fn new_instance(preset: &str, instances: &[AnimationInstance], registry: &PresetRegistry) -> Result<AnimationInstance> {
    let mut suffix = 1;
    while instances.iter().any(|i| i.id == format!("{preset}-{suffix}")) { suffix += 1; }
    Ok(AnimationInstance { id: format!("{preset}-{suffix}"), preset: preset.into(), options: registry.get(preset)?.defaults(),
        placement: Placement::Center, layer: Layer::Normal, z_index: 0, enabled: true })
}
fn normalize_startup_scene(mut scene: Scene, registry: &PresetRegistry) -> Result<Scene> {
    for instance in &mut scene.instances {
        let descriptor = registry.get(&instance.preset)?;
        loop {
            match descriptor.validate_options(&instance.options) {
                Ok(values) => { instance.options = values; break; }
                Err(AsciiAnimError::UnknownOption { option, .. } | AsciiAnimError::InvalidOptionType { option, .. }
                    | AsciiAnimError::OptionOutOfRange { option, .. } | AsciiAnimError::InvalidChoice { option, .. }
                    | AsciiAnimError::TextTooLong { option, .. }) => { instance.options.remove(&option); }
                Err(err) => return Err(err),
            }
        }
    }
    Ok(scene)
}
fn shell_quote(text: &str) -> String { format!("'{}'", text.replace('\'', "'\\''")) }

fn frame_to_text(frame: &FrameBuffer, color: bool) -> Text<'static> {
    let mut lines = Vec::with_capacity(frame.height() as usize);
    for y in 0..frame.height() {
        let mut spans = Vec::new(); let mut style = Style::default(); let mut text = String::new();
        for x in 0..frame.width() {
            let cell = frame.get(x, y).expect("frame coordinates");
            let next = if color { cell.color.map(|c| Style::default().fg(Color::Rgb(c.r, c.g, c.b))).unwrap_or_default() } else { Style::default() };
            if next != style && !text.is_empty() { spans.push(Span::styled(std::mem::take(&mut text), style)); }
            style = next; text.push(cell.ch);
        }
        spans.push(Span::styled(text, style)); lines.push(Line::from(spans));
    }
    Text::from(lines)
}

pub fn run(registry: &PresetRegistry) -> Result<()> {
    terminal::enable_raw_mode().map_err(terminal_error)?;
    let mut stdout = io::stdout();
    if let Err(err) = execute!(stdout, EnterAlternateScreen) { return restore_tui_setup(false).and(Err(terminal_error(err))); }
    let mut terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => terminal,
        Err(err) => return restore_tui_setup(true).and(Err(terminal_error(err))),
    };
    let result = (|| { let mut state = TuiState::load_startup(registry)?; run_loop(&mut terminal, registry, &mut state) })();
    let restore = restore_tui_terminal(&mut terminal);
    result.and(restore)
}
fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().map_err(|err| AsciiAnimError::Clipboard(err.to_string()))?;
    clipboard.set_text(text.to_owned()).map_err(|err| AsciiAnimError::Clipboard(err.to_string()))
}
fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, registry: &PresetRegistry, state: &mut TuiState) -> Result<()> {
    let mut previous = Instant::now();
    loop {
        let now = Instant::now(); state.advance(now.duration_since(previous)); previous = now;
        terminal.draw(|frame| draw_editor(frame, registry, state)).map_err(terminal_error)?;
        let interval = Duration::from_secs_f64(1.0 / f64::from(state.scene.frame_rate.max(1)));
        if event::poll(interval).map_err(terminal_error)? {
            match event::read().map_err(terminal_error)? {
                Event::Key(key) => {
                    let accepted_at = Instant::now();
                    state.advance(accepted_at.duration_since(previous));
                    previous = accepted_at;
                    let paused = state.is_paused();
                    let blocking_save = key.code == KeyCode::Char('s')
                        || (key.code == KeyCode::Enter && matches!(state.dialog, Dialog::Quit(0) | Dialog::Export { choice: 0, .. }));
                    match handle_tui_key(state, key, registry) {
                        Ok(TuiAction::Quit) => return Ok(()),
                        Ok(TuiAction::CopyCommand(command)) => {
                            state.set_copy_status(copy_to_clipboard(&command).map_err(|err| err.to_string()));
                            previous = Instant::now();
                        }
                        Ok(TuiAction::Continue) => {},
                        Err(err) => state.status = Some(err.to_string()),
                    }
                    if paused != state.is_paused() || blocking_save { previous = Instant::now(); }
                }
                Event::Resize(width, height) => state.resize(width, height),
                _ => {},
            }
        }
    }
}

fn panel(title: String, focused: bool) -> Block<'static> {
    Block::default().title(title).borders(Borders::ALL).border_style(Style::default().fg(if focused { AMBER } else { MUTED }))
}
fn draw_editor(frame: &mut Frame<'_>, registry: &PresetRegistry, state: &mut TuiState) {
    let area = frame.area(); state.resize(area.width, area.height);
    frame.render_widget(Block::default().style(Style::default().bg(GRAPHITE).fg(PAPER)), area);
    if editor_layout(area.width, area.height) == EditorLayout::Tiny {
        frame.render_widget(Paragraph::new("Terminal too small. Resize to at least 36 x 10.\nUnsaved work retained; resize before editing or quitting.").wrap(Wrap { trim: false }), area);
        return;
    }
    if state.fullscreen && matches!(state.dialog, Dialog::None) {
        let preview = state.preview_text(area.width.max(1), area.height.max(1));
        frame.render_widget(Paragraph::new(preview), area);
        let (canvas_width, canvas_height) = state.session.canvas_dimensions();
        if canvas_width > area.width || canvas_height > area.height {
            let warning = "Canvas cropped | f/Esc return";
            frame.render_widget(Paragraph::new(warning).style(Style::default().bg(GRAPHITE).fg(AMBER)),
                Rect::new(area.x, area.y + area.height.saturating_sub(1), area.width.min(warning.len() as u16), 1));
        }
        return;
    }
    let layout = tui_layout(area);
    let mode = editor_layout(area.width, area.height);
    let browsing = matches!(state.dialog, Dialog::Browser(_));
    let preview_area = if browsing {
        browser_panes(area).1
    } else if state.fullscreen { Rect::new(area.x, area.y + 1, area.width, area.height.saturating_sub(4)) } else { layout.preview };
    let preview = state.preview_text(preview_area.width.saturating_sub(2).max(1), preview_area.height.saturating_sub(2).max(1));
    let show_preview = browsing || state.fullscreen || mode != EditorLayout::Small || state.view == EditorView::Preview;
    if show_preview {
        let (canvas_width, canvas_height) = state.temporary.as_ref().unwrap_or(&state.session).canvas_dimensions();
        let cropped = canvas_width > preview_area.width.saturating_sub(2) || canvas_height > preview_area.height.saturating_sub(2);
        let title = format!("{}Preview{}{}", if state.focus == PaneFocus::Preview { "> " } else { "" },
            if state.temporary.is_some() { " [temporary]" } else { "" }, if cropped { " [Canvas cropped]" } else { "" });
        frame.render_widget(Paragraph::new(preview).block(panel(title, state.focus == PaneFocus::Preview)), preview_area);
    }
    if !state.fullscreen && !browsing {
        match mode {
            EditorLayout::Wide => {
                let list_height = (layout.options.height / 4).clamp(5, 9);
                draw_scene(frame, state, Rect::new(layout.options.x, layout.options.y, layout.options.width, list_height));
                draw_inspector(frame, state, Rect::new(layout.options.x, layout.options.y + list_height, layout.options.width, layout.options.height.saturating_sub(list_height)));
            }
            EditorLayout::Medium => {
                if state.focus == PaneFocus::Scene { draw_scene(frame, state, layout.options); }
                else { draw_inspector(frame, state, layout.options); }
            }
            EditorLayout::Small => match state.view { EditorView::Scene => draw_scene(frame, state, layout.options), EditorView::Edit => draw_inspector(frame, state, layout.options), EditorView::Preview => {} },
            EditorLayout::Tiny => {},
        }
    }
    let dirty = if state.is_dirty() { "* unsaved" } else { "saved" };
    let playback = if state.is_paused() { "paused" } else { "playing" };
    let focused = match state.focus { PaneFocus::Preview => "Preview", PaneFocus::Scene => "Scene", PaneFocus::Inspector => "Edit" };
    let heading = if area.width < 80 { format!("ASCII {dirty} {playback} [{focused}]") }
        else { format!("ASCII Animation {dirty} {playback} [{} / {} / {}]",
            if state.focus == PaneFocus::Preview { ">Preview" } else { "Preview" },
            if state.focus == PaneFocus::Scene { ">Scene" } else { "Scene" },
            if state.focus == PaneFocus::Inspector { ">Edit" } else { "Edit" }) };
    frame.render_widget(Paragraph::new(heading).style(Style::default().fg(AMBER)), Rect::new(area.x, area.y, area.width, 1));
    let hints = match &state.dialog {
        Dialog::Browser(_) => "Up/Down select Enter add Esc cancel",
        Dialog::Editor(_) => "Enter commit Esc cancel Home/End",
        Dialog::ConfirmDelete | Dialog::ConfirmReplace(_) => "Enter confirm Esc cancel",
        Dialog::Quit(_) => "Enter choose Esc cancel",
        Dialog::Export { .. } => "Tab choose Enter confirm Esc close",
        Dialog::Help(_) => "Up/Down scroll Esc close",
        Dialog::Recovery(_) => "r reload d default q quit",
        Dialog::None if area.width < 80 => match state.focus {
            PaneFocus::Scene => "Tab focus Up/Down select Enter edit",
            PaneFocus::Inspector => "Arrows adjust Enter edit Tab focus",
            PaneFocus::Preview => "Tab focus Space pause f fullscreen",
        },
        Dialog::None => match state.focus {
            PaneFocus::Scene => "Up/Down select Enter edit d delete r replace [/] move",
            PaneFocus::Inspector => "Up/Down field Left/Right adjust Shift: fast Enter edit",
            PaneFocus::Preview => "Tab: Scene/Edit Space: pause f: fullscreen",
        },
    };
    let status = if matches!(state.dialog, Dialog::Recovery(_)) { "Original file is unchanged" }
        else if matches!(state.dialog, Dialog::Browser(_)) { "Enter chooses; PgUp/PgDn details" }
        else if matches!(state.dialog, Dialog::Export { .. }) {
            state.copy_status.as_deref().or(state.status.as_deref()).unwrap_or("Up/Down/PageUp/PageDown: command")
        } else { state.status().unwrap_or("Ready") };
    let global = match &state.dialog {
        Dialog::Recovery(_) => "Up/Down/PageUp/PageDown: details".to_string(),
        Dialog::Browser(_) | Dialog::Editor(_) => "Printable keys type; Esc cancels".to_string(),
        Dialog::Export { choice, .. } => selected_buttons(
            &[if state.scene.requires_config_export() && state.is_dirty() { "Save and Copy" } else { "Copy" }, "Cancel"], *choice),
        Dialog::Quit(choice) => selected_buttons(&["Save", "Discard", "Cancel"], *choice),
        _ if area.width < 80 => "a add s save c export ? help q quit".to_string(),
        _ => "Tab focus  a add  s save  c export  ? help  q quit".to_string(),
    };
    frame.render_widget(Paragraph::new(vec![Line::from(status), Line::from(hints), Line::from(global)]).style(Style::default().fg(MUTED)), Rect::new(area.x, area.y + area.height.saturating_sub(3), area.width, area.height.min(3)));
    draw_dialog(frame, state, registry, area);
}
fn keep_visible(scroll: &mut usize, selected: usize, count: usize) {
    let count = count.max(1);
    if selected < *scroll { *scroll = selected; }
    else if selected >= scroll.saturating_add(count) { *scroll = selected + 1 - count; }
}
fn draw_scene(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    keep_visible(&mut state.scene_scroll, state.selected_instance, area.height.saturating_sub(2) as usize);
    let lines: Vec<_> = state.scene.instances.iter().enumerate().skip(state.scene_scroll).map(|(index, instance)| {
        Line::styled(format!("{} [{}] {} ({})", if index == state.selected_instance { ">" } else { " " }, if instance.enabled { "on" } else { "off" }, instance.id, instance.preset),
            Style::default().fg(if index == state.selected_instance && state.focus == PaneFocus::Scene { AMBER } else { PAPER }))
    }).collect();
    frame.render_widget(Paragraph::new(lines).block(panel(format!("Scene ({})", state.scene.instances.len()), state.focus == PaneFocus::Scene)), area);
}
fn draw_inspector(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    let mut lines = Vec::new(); let mut group = None; let mut selected_row = 0;
    for (index, field) in state.fields.iter().enumerate() {
        if group != Some(group_index(field.group)) {
            lines.push(Line::styled(group_label(field.group), Style::default().fg(MUTED))); group = Some(group_index(field.group));
        }
        if index == state.selected_option { selected_row = lines.len(); }
        let value = state.field_value(&field.name).map(|v| format_tui_option_value(&v)).unwrap_or_default();
        lines.push(Line::styled(format!("{} {}: {}{}", if index == state.selected_option { ">" } else { " " }, field.label, value, if field.rebuilds { " [restart]" } else { "" }),
            Style::default().fg(if index == state.selected_option && state.focus == PaneFocus::Inspector { AMBER } else { PAPER })));
    }
    let help_height = if area.height >= 10 { 3 } else { 2 };
    let content = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(help_height));
    keep_visible(&mut state.inspector_scroll, selected_row, content.height.saturating_sub(2) as usize);
    frame.render_widget(Paragraph::new(lines).scroll((state.inspector_scroll.min(u16::MAX as usize) as u16, 0))
        .block(panel(format!("Edit: {}", state.selected_instance().id), state.focus == PaneFocus::Inspector)), content);
    if let Some(field) = state.fields.get(state.selected_option) {
        let range = match &field.kind { OptionKind::Int { min, max, step } => format!("{min}..{max}; step {step}"),
            OptionKind::Float { min, max } => format!("{min}..{max}"), OptionKind::Text { max_len } => format!("ASCII; max {max_len}"),
            OptionKind::Bool => "on/off".into(), OptionKind::Choice { .. } => "Enter: choices; Escape: cancel".into() };
        let flag = field.cli_flag.as_deref().unwrap_or(&field.name);
        frame.render_widget(Paragraph::new(format!("{flag}  {range}\n{}", field.help)).style(Style::default().fg(MUTED)).wrap(Wrap { trim: false }),
            Rect::new(area.x + 1, content.y + content.height, area.width.saturating_sub(2), help_height));
    }
}
fn modal_area(area: Rect, width: u16, height: u16) -> Rect {
    let body = Rect::new(area.x, area.y.saturating_add(1), area.width, area.height.saturating_sub(4));
    let width = width.min(body.width); let height = height.min(body.height.max(1));
    Rect::new(body.x + body.width.saturating_sub(width) / 2, body.y + body.height.saturating_sub(height) / 2, width, height)
}
fn browser_panes(area: Rect) -> (Rect, Rect) {
    let layout = tui_layout(area);
    match editor_layout(area.width, area.height) {
        EditorLayout::Wide | EditorLayout::Medium => (layout.options, layout.preview),
        _ if area.height < 14 => {
            let width = layout.options.width / 2;
            (Rect::new(layout.options.x, layout.options.y, width, layout.options.height),
             Rect::new(layout.preview.x + width, layout.preview.y, layout.preview.width - width, layout.preview.height))
        }
        _ => {
            let height = layout.preview.height / 2;
            (Rect::new(layout.options.x, layout.options.y + height, layout.options.width, layout.options.height - height),
             Rect::new(layout.preview.x, layout.preview.y, layout.preview.width, height))
        }
    }
}
fn draw_dialog(frame: &mut Frame<'_>, state: &TuiState, registry: &PresetRegistry, area: Rect) {
    if matches!(state.dialog, Dialog::None) { return; }
    let modal = if matches!(state.dialog, Dialog::Browser(_)) { browser_panes(area).0 }
        else { modal_area(area, 82, if matches!(state.dialog, Dialog::Export { .. } | Dialog::Help(_) | Dialog::Recovery(_)) { 18 } else { 10 }) };
    frame.render_widget(Clear, modal);
    frame.render_widget(Block::default().style(Style::default().bg(GRAPHITE).fg(PAPER)), modal);
    if let Dialog::Browser(browser) = &state.dialog {
        let border = panel(if browser.replace { "Replace Preset".into() } else { "Add Preset".into() }, true);
        let inner = border.inner(modal);
        frame.render_widget(border, modal);
        frame.render_widget(Paragraph::new(format!("Find: {}", cursor_window(&browser.search, browser.cursor, inner.width.saturating_sub(7) as usize))),
            Rect::new(inner.x, inner.y, inner.width, 1));
        let description_height = inner.height.saturating_sub(2).min(4).max(1);
        let rows = inner.height.saturating_sub(1 + description_height).max(1) as usize;
        let start = browser.selected.saturating_sub(rows - 1);
        let lines: Vec<_> = browser.names.iter().enumerate().skip(start).take(rows).map(|(index, name)| {
            let descriptor = registry.get(name).expect("browser descriptor");
            let label = if inner.width < 30 { name.clone() } else { format!("{name} - {}", descriptor.label()) };
            Line::styled(format!("{} {label}", if index == browser.selected { ">" } else { " " }),
                Style::default().fg(if index == browser.selected { AMBER } else { PAPER }))
        }).collect();
        frame.render_widget(Paragraph::new(lines), Rect::new(inner.x, inner.y + 1, inner.width, rows as u16));
        let description = browser.names.get(browser.selected).map(|name| {
            let descriptor = registry.get(name).expect("browser descriptor");
            format!("{}: {}", descriptor.label(), descriptor.description())
        }).unwrap_or_else(|| "No matches. Edit search or Escape.".into());
        frame.render_widget(Paragraph::new(description).wrap(Wrap { trim: false }).scroll((browser.description_scroll, 0)).style(Style::default().fg(MUTED)),
            Rect::new(inner.x, inner.y + 1 + rows as u16, inner.width, description_height));
        return;
    }
    let (title, text, scroll) = match &state.dialog {
        Dialog::Browser(_) => unreachable!("browser is rendered with separate description space"),
        Dialog::Editor(draft) => {
            let field = state.fields.iter().find(|f| f.name == draft.name).unwrap();
            let value = match &draft.value { OptionValue::Text(text) => cursor_window(text, draft.cursor, modal.width.saturating_sub(4) as usize), other => format_tui_option_value(other) };
            (format!("Edit {}", field.label), Text::from(format!("{value}\n{}\nEnter commit; Esc cancel\nArrows/Home/End; Backspace/Delete", draft.error.as_deref().unwrap_or(&field.help))), 0)
        }
        Dialog::ConfirmDelete => ("Delete animation instance?".into(), Text::from(format!("Enter/y delete; Esc/n cancel\nRemove {} and its configured options?", state.selected_instance().id)), 0),
        Dialog::ConfirmReplace(name) => ("Replace configured Preset?".into(), Text::from(format!("Enter/y replace; Esc/n cancel\nReplace {} with {name}? Prior Preset options are lost.\nPlacement, Layer and Z-index remain.", state.selected_instance().id)), 0),
        Dialog::Quit(choice) => ("Unsaved Scene".into(), Text::from(format!("Save changes before quitting?\n{}\nLeft/Right select; Enter confirms; Escape cancels\n{}", selected_buttons(&["Save", "Discard", "Cancel"], *choice), state.status.as_deref().unwrap_or(""))), 0),
        Dialog::Export { scroll, choice } => {
            let label = if state.scene.requires_config_export() && state.is_dirty() { "Save and Copy" } else { "Copy" };
            ("Export committed Scene".into(), Text::from(format!("{}\n\nConfig: {}\n{}\n{}\n{}\n{}\nUp/Down/PageUp/PageDown scroll; Enter confirms; Escape closes",
                state.export_command(), state.config_path.display(), state.export_status().unwrap_or_default(),
                selected_buttons(&[label, "Cancel"], *choice), state.copy_status.as_deref().unwrap_or(""), state.status.as_deref().unwrap_or(""))), *scroll)
        }
        Dialog::Help(scroll) => ("Keyboard help".into(), Text::from("Tab / Shift+Tab: focus Preview, Scene, Edit\nUp/Down: select instance or inspector field\nLeft/Right: adjust; Shift: faster numeric adjustment\nEnter: edit text/choice; Enter commits; Escape cancels\nText: Left/Right, Home/End, Backspace/Delete\nSpace: pause/resume without restart\nf: fullscreen preview\na: searchable Add Preset browser\nScene: d delete, r replace (confirmed), [/] reorder\nEdit Layout: enabled, Placement, Layer, Z-index\ns: save Scene; c: export panel (committed values only)\nExport: Save and Copy saves config before copying\nq: quit; unsaved changes offer Save/Discard/Cancel\nSmall terminal: Tab switches Preview/Scene/Edit views\n[restart] marks options that rebuild selected state\n[Canvas cropped] means only part of Canvas is visible\nEscape closes transient surfaces; never quits editor"), *scroll),
        Dialog::Recovery(scroll) => ("Cannot open saved Scene".into(), Text::from(format!("Original file is unchanged.\nr: Reload   d/Enter: Unsaved default   q: Quit\n\nConfig: {}\n\n{}", state.config_path.display(), state.startup_error.as_deref().unwrap_or("Invalid Scene"))), *scroll),
        Dialog::None => unreachable!(),
    };
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }).scroll((scroll, 0)).block(panel(title, true)), modal);
}
fn cursor_window(text: &str, cursor: usize, width: usize) -> String {
    let cursor = cursor.min(text.len());
    let width = width.max(2);
    let before = text[..cursor].chars().count();
    let skip = before.saturating_sub(width - 1);
    let start = text.char_indices().nth(skip).map(|(index, _)| index).unwrap_or(text.len());
    let end = text[start..].char_indices().nth(width - 1).map(|(index, _)| start + index).unwrap_or(text.len());
    format!("{}|{}", &text[start..cursor], &text[cursor..end])
}
fn selected_buttons(labels: &[&str], choice: usize) -> String {
    labels.iter().enumerate().map(|(index, label)| if index == choice { format!("[>{label}<]") } else { format!("[{label}]") }).collect::<Vec<_>>().join("  ")
}
fn restore_tui_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let restore = execute!(terminal.backend_mut(), Show, LeaveAlternateScreen).err();
    finish_tui_restore(restore, terminal::disable_raw_mode().err())
}
fn restore_tui_setup(entered: bool) -> Result<()> {
    let restore = if entered { execute!(io::stdout(), Show, LeaveAlternateScreen).err() } else { None };
    finish_tui_restore(restore, terminal::disable_raw_mode().err())
}
fn finish_tui_restore(restore: Option<io::Error>, disable: Option<io::Error>) -> Result<()> {
    match (restore, disable) { (None, None) => Ok(()), (Some(err), None) | (None, Some(err)) => Err(terminal_error(err)),
        (Some(a), Some(b)) => Err(AsciiAnimError::Terminal(format!("{a}; additionally failed to disable raw mode: {b}"))) }
}
fn terminal_error(err: io::Error) -> AsciiAnimError { AsciiAnimError::Terminal(err.to_string()) }
