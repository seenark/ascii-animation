use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use crossterm::cursor::Show;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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
pub enum Surface { None, Browser, SavedScenes, Naming, Editor, Confirmation, Quit, Export, Help, Recovery, SaveError }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiAction { Continue, Quit, CopyCommand(String) }
/// Terminal input, controlled elapsed time, and adapter feedback share one application boundary.
pub enum TuiEvent { Terminal(Event), Advance(Duration), Clipboard(std::result::Result<(), String>) }
impl From<Event> for TuiEvent {
    fn from(event: Event) -> Self { Self::Terminal(event) }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuiLayout { pub options: Rect, pub preview: Rect }
pub fn tui_layout(area: Rect) -> TuiLayout {
    let (options, preview) = split_tui_layout(area);
    TuiLayout { options, preview }
}

#[derive(Clone)]
struct Field { name: String, label: String, kind: OptionKind, group: OptionGroup, help: String, rebuilds: bool }
struct Draft { name: String, value: OptionValue, cursor: usize, error: Option<String> }
struct Browser { search: String, cursor: usize, names: Vec<String>, selected: usize, purpose: BrowserPurpose, typing: bool, previewing: bool, description_scroll: u16 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum BrowserPurpose { New, Add, Replace }
struct SavedEntry { name: String, path: PathBuf }
struct SavedBrowser { entries: Vec<SavedEntry>, selected: usize, scroll: usize, previewing: bool, needs_save: bool, directory_error: Option<String>, error: Option<String> }
#[derive(Clone)]
enum Transition { New(String), Open(PathBuf, String), Quit }
enum AfterSave { Stay, Copy, Transition(Transition) }
struct SaveRequest { path: PathBuf, name: String, overwrite: bool, after: AfterSave }
#[derive(Clone)]
enum HitAction { Key(KeyCode), HelpAction(KeyCode), Preset(usize), Saved(usize), Instance(usize), Field(usize), Focus(PaneFocus) }
struct HitTarget { area: Rect, action: HitAction }
enum Dialog {
    None,
    Browser(Browser),
    Saved(SavedBrowser),
    Name { text: String, cursor: usize, error: Option<String>, after: AfterSave },
    Overwrite(SaveRequest),
    SaveError { request: SaveRequest, error: String, scroll: u16 },
    Guard { choice: usize, transition: Transition },
    Editor(Draft),
    ConfirmDelete,
    ConfirmReplace(String),
    Export { scroll: u16, choice: usize, command: Option<String> },
    Help(u16),
    Recovery(u16),
    ReadError { error: String, scroll: u16 },
}
struct Underlay { dialog: Dialog, preview: Option<SceneSession>, focused_action: Option<usize> }

pub struct TuiState {
    pub scene: Scene,
    session: SceneSession,
    entry_ids: Vec<EntryId>,
    saved_scene: Option<Scene>,
    config_path: PathBuf,
    selected_instance: usize,
    default_path: PathBuf,
    saved_name: Option<String>,
    editor_active: bool,
    underlays: Vec<Underlay>,
    playback_dialog: Option<Dialog>,
    hit_targets: Vec<HitTarget>,
    focused_action: Option<usize>,
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
        let config_path = absolute_path(&config_path)?;
        let session = SceneSession::new(scene, registry, 0)?;
        let scene = session.scene().clone();
        let entry_ids = session.entry_ids();
        let custom_placements = scene.instances.iter().map(|i| match i.placement {
            Placement::Custom { .. } => i.placement.clone(), _ => default_custom_placement(),
        }).collect();
        let mut state = Self { scene, session, entry_ids, saved_scene, default_path: config_path.clone(), config_path, saved_name: None, editor_active: true,
            underlays: Vec::new(), playback_dialog: None, hit_targets: Vec::new(), focused_action: None, selected_instance: 0,
            selected_option: 0, fields: Vec::new(), option_names: Vec::new(), custom_placements,
            focus: PaneFocus::Preview, return_focus: PaneFocus::Preview, view: EditorView::Preview,
            fullscreen: false, fullscreen_focus: None, dialog: Dialog::None, temporary: None, status: None, copy_status: None,
            startup_error: None, scene_scroll: 0, inspector_scroll: 0, terminal_size: (120, 38) };
        state.sync_selected_options(registry)?;
        Ok(state)
    }
    pub fn load_startup(registry: &PresetRegistry) -> Result<Self> {
        Self::startup_at(Scene::default_config_path(), registry)
    }
    pub fn startup_at(path: impl AsRef<Path>, registry: &PresetRegistry) -> Result<Self> {
        let mut state = Self::default_with_registry(registry)?;
        state.default_path = absolute_path(path.as_ref())?;
        state.config_path = state.default_path.clone();
        state.editor_active = false;
        state.browse_presets(BrowserPurpose::New, registry)?;
        Ok(state)
    }
    pub fn load_from_path(path: impl AsRef<Path>, registry: &PresetRegistry) -> Result<Self> {
        let path = absolute_path(path.as_ref())?;
        let loaded = Scene::load_from_path_raw(&path);
        match loaded {
            Ok(baseline) => {
                match normalize_startup_scene(baseline.clone(), registry)
                    .and_then(|scene| Self::new(scene, Some(baseline), path.clone(), registry)) {
                    Ok(mut state) => { state.saved_name = Some(path.file_stem().unwrap_or_default().to_string_lossy().into_owned()); Ok(state) },
                    Err(err) => Self::recovery(path, err.to_string(), registry),
                }
            }
            Err(_) if matches!(path.try_exists(), Ok(false)) => {
                let mut state = Self::default_with_registry(registry)?;
                state.default_path = path.clone();
                state.config_path = path;
                Ok(state)
            }
            Err(err) => Self::recovery(path, err.to_string(), registry),
        }
    }
    fn recovery(path: PathBuf, error: String, registry: &PresetRegistry) -> Result<Self> {
        let mut state = Self::default_with_registry(registry)?;
        state.default_path = path.clone();
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
        if let Dialog::Name { error: Some(error), .. } = &self.dialog { return Some(error); }
        if let Dialog::SaveError { error, .. } = &self.dialog { return Some(error); }
        self.status.as_deref()
    }
    pub fn copy_status(&self) -> Option<&str> { self.copy_status.as_deref() }
    pub fn surface(&self) -> Surface {
        match self.dialog { Dialog::None => Surface::None, Dialog::Browser(_) => Surface::Browser, Dialog::Saved(_) => Surface::SavedScenes,
            Dialog::Name { .. } => Surface::Naming, Dialog::SaveError { .. } => Surface::SaveError,
            Dialog::Editor(_) => Surface::Editor, Dialog::ConfirmDelete | Dialog::ConfirmReplace(_) | Dialog::Overwrite(_) => Surface::Confirmation,
            Dialog::Guard { .. } => Surface::Quit, Dialog::Export { .. } => Surface::Export,
            Dialog::Help(_) => Surface::Help, Dialog::Recovery(_) | Dialog::ReadError { .. } => Surface::Recovery }
    }
    pub fn draft_text(&self) -> Option<&str> {
        match &self.dialog { Dialog::Editor(Draft { value: OptionValue::Text(value), .. }) => Some(value),
            Dialog::Browser(browser) => Some(&browser.search), Dialog::Name { text, .. } => Some(text), _ => None }
    }
    pub fn selected_option_name(&self) -> Option<&str> { self.option_names.get(self.selected_option).map(String::as_str) }
    pub fn selected_instance_index(&self) -> usize { self.selected_instance }
    pub fn selected_instance(&self) -> &AnimationInstance { &self.scene.instances[self.selected_instance] }
    pub fn visible_option_names(&self) -> &[String] { &self.option_names }
    pub fn editing_text(&self) -> bool { matches!(&self.dialog, Dialog::Editor(Draft { value: OptionValue::Text(_), .. })) }
    pub fn selected_option_is_text(&self) -> bool { matches!(self.fields.get(self.selected_option).map(|f| &f.kind), Some(OptionKind::Text { .. })) }
    fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.focused_action = None;
        self.hit_targets.clear();
        if self.fullscreen {
            self.fullscreen_focus = Some(self.focus);
            self.focus = PaneFocus::Preview;
            if !matches!(self.dialog, Dialog::None) {
                self.playback_dialog = Some(std::mem::replace(&mut self.dialog, Dialog::None));
            }
        } else {
            if let Some(focus) = self.fullscreen_focus.take() { self.focus = focus; }
            if let Some(dialog) = self.playback_dialog.take() { self.dialog = dialog; }
        }
    }
    pub fn resize(&mut self, width: u16, height: u16) {
        if self.terminal_size != (width, height) { self.hit_targets.clear(); }
        self.terminal_size = (width, height);
    }
    fn active_preview(&self) -> &SceneSession {
        self.temporary.as_ref().or_else(|| self.underlays.last().and_then(|underlay| underlay.preview.as_ref())).unwrap_or(&self.session)
    }
    pub fn advance(&mut self, delta: Duration) {
        self.session.advance(delta);
        if let Some(temporary) = &mut self.temporary {
            temporary.set_paused(self.session.is_paused());
            temporary.advance(delta);
        }
        for underlay in &mut self.underlays {
            if let Some(preview) = &mut underlay.preview {
                preview.set_paused(self.session.is_paused());
                preview.advance(delta);
            }
        }
    }
    pub fn preview_text(&mut self, width: u16, height: u16) -> Text<'static> {
        // Draw the live session even behind a draft so stateful entries continue advancing.
        let live = self.session.draw(width, height);
        match live {
            Err(err) => { self.status = Some(err.to_string()); Text::from(err.to_string()) }
            Ok(frame) => {
                if matches!(self.dialog, Dialog::Saved(_)) && self.temporary.is_none() {
                    return Text::from("Preview unavailable. Select a readable Saved Scene or choose New. e shows error details; r retries reading.");
                }
                if matches!(&self.dialog, Dialog::Browser(browser) if browser.names.is_empty()) {
                    return Text::from("No matching Preset. Edit search or Escape to return.");
                }
                let temporary = self.temporary.as_mut().or_else(|| self.underlays.last_mut().and_then(|underlay| underlay.preview.as_mut()));
                if let Some(temporary) = temporary {
                    let color = temporary.scene().color;
                    match temporary.draw(width, height) {
                        Ok(frame) => frame_to_text(frame, color),
                        Err(err) => { self.status = Some(err.to_string()); Text::from(err.to_string()) }
                    }
                } else { frame_to_text(frame, self.scene.color) }
            }
        }
    }
    pub fn export_command(&self) -> String {
        format!("ascii-animation run --config {}", shell_quote(&self.config_path.to_string_lossy()))
    }
    pub fn export_status(&self) -> Option<String> {
        self.is_dirty().then(|| "Save and Copy writes the complete configuration before copying. No runtime history is captured.".into())
    }
    fn set_copy_status(&mut self, result: std::result::Result<(), String>) {
        self.copy_status = Some(match result { Ok(()) => "Copied command to clipboard".into(), Err(message) => format!("Copy failed: {message}; command remains available") });
    }
    fn open(&mut self, dialog: Dialog) { self.return_focus = self.focus; self.dialog = dialog; self.copy_status = None; self.focused_action = None; self.hit_targets.clear(); }
    fn overlay(&mut self, dialog: Dialog) {
        self.underlays.push(Underlay { dialog: std::mem::replace(&mut self.dialog, Dialog::None), preview: self.temporary.take(), focused_action: self.focused_action });
        self.open(dialog);
    }
    fn close(&mut self) {
        self.focused_action = None;
        if let Some(underlay) = self.underlays.pop() {
            self.dialog = underlay.dialog;
            self.temporary = underlay.preview;
            self.focused_action = underlay.focused_action;
        } else {
            self.dialog = Dialog::None;
            if !self.fullscreen { self.temporary = None; }
        }
        self.focus = self.return_focus;
        
        self.hit_targets.clear();
    }
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
        match (&*field.name, next) {
            ("frame-rate", OptionValue::Int(value)) => scene.frame_rate = value as u16,
            ("color", OptionValue::Bool(value)) => scene.color = value,
            (_, value) => set_field(&mut scene.instances[self.selected_instance], &field.name, value, &self.custom_placements[self.selected_instance]),
        }
        self.commit(scene, self.identities(), registry)?;
        if matches!(self.selected_instance().placement, Placement::Custom { .. }) {
            self.custom_placements[self.selected_instance] = self.selected_instance().placement.clone();
        }
        Ok(())
    }
    fn field_value(&self, name: &str) -> Option<OptionValue> {
        let instance = self.selected_instance();
        match name {
            "frame-rate" => Some(OptionValue::Int(self.scene.frame_rate as i64)),
            "color" => Some(OptionValue::Bool(self.scene.color)),
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
            name: o.name().into(), label: o.label().into(), kind: o.kind().clone(), group: o.group(), help: o.help().into(), rebuilds: o.rebuilds_state(),
        }).collect();
        fields.push(Field { name: "frame-rate".into(), label: "Frame rate".into(), kind: OptionKind::Int { min: 1, max: 240, step: 1 }, group: OptionGroup::Motion, help: "Shared frames per second for every animation.".into(), rebuilds: false });
        fields.push(Field { name: "color".into(), label: "Color".into(), kind: OptionKind::Bool, group: OptionGroup::Style, help: "Shared color output; disabling color preserves animation cells.".into(), rebuilds: false });
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
        self.browse_presets(if replace { BrowserPurpose::Replace } else { BrowserPurpose::Add }, registry)
    }
    fn browse_presets(&mut self, purpose: BrowserPurpose, registry: &PresetRegistry) -> Result<()> {
        self.open(Dialog::Browser(Browser { search: String::new(), cursor: 0, names: Vec::new(), selected: 0, purpose, typing: purpose != BrowserPurpose::New, previewing: false, description_scroll: 0 }));
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
    fn library_path(&self) -> PathBuf {
        self.default_path.parent().unwrap_or(Path::new(".")).join("saved-scenes")
    }
    fn browse_saved(&mut self, registry: &PresetRegistry) -> Result<()> {
        let mut entries = Vec::new();
        let mut error = None;
        match std::fs::read_dir(self.library_path()) {
            Ok(files) => for file in files {
                match file {
                    Ok(file) => {
                        let path = file.path();
                        if path.extension().is_some_and(|ext| ext == "toml") {
                            let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                            entries.push(SavedEntry { name, path });
                        }
                    }
                    Err(err) => error = Some(format!("Cannot read Saved Scenes: {err}. r retries; Esc returns.")),
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => {},
            Err(err) => error = Some(format!("Cannot read Saved Scenes: {err}. Check directory permissions; r retries; Esc returns.")),
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        if self.default_path.try_exists().unwrap_or(true) {
            entries.push(SavedEntry { name: "Default (existing configuration)".into(), path: self.default_path.clone() });
        }
        self.overlay(Dialog::Saved(SavedBrowser { entries, selected: 0, scroll: 0, previewing: false, needs_save: false, directory_error: error, error: None }));
        self.refresh_saved(registry);
        Ok(())
    }
    fn refresh_saved(&mut self, registry: &PresetRegistry) {
        self.temporary = None;
        let Dialog::Saved(browser) = &mut self.dialog else { return; };
        let Some(entry) = browser.entries.get(browser.selected) else { return; };
        browser.needs_save = false;
        match Scene::load_from_path_raw(&entry.path).and_then(|baseline| {
            let scene = normalize_startup_scene(baseline.clone(), registry)?;
            let needs_save = scene != baseline;
            Ok((SceneSession::new(scene, registry, 0)?, needs_save))
        }) {
            Ok((mut session, needs_save)) => {
                session.set_paused(self.session.is_paused());
                self.temporary = Some(session);
                browser.needs_save = needs_save;
                browser.error = needs_save.then(|| "Saved configuration needs updating. Open/Edit, then Save before Play or Copy. File unchanged.".into());
            }
            Err(err) => browser.error = Some(format!("Cannot open {}: {err}. File unchanged. r retries; select another entry or Esc returns.", entry.name)),
        }
    }
    fn request_transition(&mut self, transition: Transition, registry: &PresetRegistry) -> Result<TuiAction> {
        if self.editor_active && self.is_dirty() {
            self.overlay(Dialog::Guard { choice: 2, transition });
            Ok(TuiAction::Continue)
        } else { self.transition(transition, registry) }
    }
    fn transition(&mut self, transition: Transition, registry: &PresetRegistry) -> Result<TuiAction> {
        let (scene, path, name, saved) = match transition {
            Transition::Quit => return Ok(TuiAction::Quit),
            Transition::New(preset) => (Scene { frame_rate: 30, color: true, instances: vec![new_instance(&preset, &[], registry)?] }, self.default_path.clone(), None, None),
            Transition::Open(path, name) => {
                match Scene::load_from_path_raw(&path).and_then(|baseline| {
                    let scene = normalize_startup_scene(baseline.clone(), registry)?;
                    Ok((scene, baseline))
                }) {
                    Ok((scene, baseline)) => (scene, path, Some(name), Some(baseline)),
                    Err(err) => {
                        self.overlay(Dialog::ReadError { error: format!("Cannot open {name}: {err}. File unchanged. Esc returns."), scroll: 0 });
                        return Ok(TuiAction::Continue);
                    }
                }
            },
        };
        let default_path = self.default_path.clone();
        let size = self.terminal_size;
        let mut next = match Self::new(scene, saved, path, registry) {
            Ok(next) => next,
            Err(err) => {
                self.overlay(Dialog::ReadError { error: format!("Cannot open Scene: {err}. Current editor retained. Esc returns."), scroll: 0 });
                return Ok(TuiAction::Continue);
            }
        };
        next.default_path = default_path;
        next.saved_name = name;
        if next.is_dirty() && next.saved_scene.is_some() {
            next.status = Some("Saved options updated in memory. Save before Play or Copy; file unchanged.".into());
        }
        next.resize(size.0, size.1);
        next.focus = PaneFocus::Inspector;
        next.view = EditorView::Edit;
        *self = next;
        Ok(TuiAction::Continue)
    }
    fn request_save(&mut self, save_as: bool, after: AfterSave, registry: &PresetRegistry) -> Result<TuiAction> {
        if save_as || self.saved_name.is_none() {
            self.overlay(Dialog::Name { text: String::new(), cursor: 0, error: None, after });
            Ok(TuiAction::Continue)
        } else {
            self.persist(SaveRequest { path: self.config_path.clone(), name: self.saved_name.clone().unwrap(), overwrite: true, after }, registry)
        }
    }
    fn persist(&mut self, request: SaveRequest, registry: &PresetRegistry) -> Result<TuiAction> {
        if !request.overwrite {
            match std::fs::symlink_metadata(&request.path) {
                Ok(_) => { self.open(Dialog::Overwrite(request)); return Ok(TuiAction::Continue); },
                Err(err) if err.kind() == io::ErrorKind::NotFound => {},
                Err(err) => {
                    self.open(Dialog::SaveError { request, error: format!("Cannot inspect save target: {err}. Check directory permissions. r retries; Esc cancels."), scroll: 0 });
                    return Ok(TuiAction::Continue);
                }
            }
        }
        match self.scene.save_to_path(&request.path) {
            Err(err) => {
                self.open(Dialog::SaveError { request, error: format!("Save failed: {err}. Check permissions and available disk space. r/Enter retries; Esc cancels."), scroll: 0 });
                Ok(TuiAction::Continue)
            }
            Ok(()) => {
                self.config_path = request.path;
                self.saved_name = Some(request.name);
                self.saved_scene = Some(self.scene.clone());
                self.status = Some("Saved complete Scene configuration; runtime history is not captured".into());
                self.underlays.clear();
                self.dialog = Dialog::None;
                self.focused_action = None;
                self.temporary = None;
                
                match request.after {
                    AfterSave::Stay => Ok(TuiAction::Continue),
                    AfterSave::Copy => {
                        let command = self.export_command();
                        self.open(Dialog::Export { scroll: 0, choice: 0, command: Some(command.clone()) });
                        Ok(TuiAction::CopyCommand(command))
                    }
                    AfterSave::Transition(transition) => self.transition(transition, registry),
                }
            }
        }
    }
}

/// Handles terminal input at the same boundary used by the executable and test backends.
pub fn handle_tui_event(state: &mut TuiState, event: impl Into<TuiEvent>, registry: &PresetRegistry) -> Result<TuiAction> {
    match event.into() {
        TuiEvent::Terminal(Event::Key(key)) => handle_key(state, key, registry),
        TuiEvent::Terminal(Event::Mouse(mouse)) => handle_mouse(state, mouse, registry),
        TuiEvent::Terminal(Event::Resize(width, height)) => { state.resize(width, height); Ok(TuiAction::Continue) },
        TuiEvent::Advance(delta) => { state.advance(delta); Ok(TuiAction::Continue) },
        TuiEvent::Clipboard(result) => { state.set_copy_status(result); Ok(TuiAction::Continue) },
        _ => Ok(TuiAction::Continue),
    }
}

fn handle_key(state: &mut TuiState, key: KeyEvent, registry: &PresetRegistry) -> Result<TuiAction> {
    if key.kind == KeyEventKind::Release { return Ok(TuiAction::Continue); }
    if editor_layout(state.terminal_size.0, state.terminal_size.1) == EditorLayout::Tiny {
        if key.code == KeyCode::F(1) && !matches!(state.dialog, Dialog::Help(_)) { state.overlay(Dialog::Help(0)); }
        else if key.code == KeyCode::Esc && matches!(state.dialog, Dialog::Help(_)) { state.close(); }
        return Ok(TuiAction::Continue);
    }
    let typing = matches!(&state.dialog, Dialog::Name { .. } | Dialog::Editor(Draft { value: OptionValue::Text(_), .. }))
        || matches!(&state.dialog, Dialog::Browser(browser) if browser.typing);
    if key.code == KeyCode::F(1) || (key.code == KeyCode::Char('?') && !typing && !matches!(state.dialog, Dialog::Help(_))) {
        state.overlay(Dialog::Help(0));
        return Ok(TuiAction::Continue);
    }
    if state.fullscreen && matches!(state.dialog, Dialog::None) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('f') => state.toggle_fullscreen(),
            KeyCode::Char(' ') => state.session.set_paused(!state.session.is_paused()),
            KeyCode::Char('q') => return state.request_transition(Transition::Quit, registry),
            _ => {},
        }
        return Ok(TuiAction::Continue);
    }
    let action_navigation = !matches!(state.dialog, Dialog::None | Dialog::Browser(_) | Dialog::Help(_) | Dialog::Guard { .. } | Dialog::Export { .. });
    if typing && matches!(key.code, KeyCode::Char(_)) { state.focused_action = None; }
    if action_navigation && (matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
        || (state.focused_action.is_some() && matches!(key.code, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::Enter))) {
        let mut actions = available_actions(state, false);
        let action_count = actions.clone().count();
        if action_count > 0 {
            match key.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    let next = match (state.focused_action, key.code) {
                        (None, KeyCode::Tab) => Some(0),
                        (None, _) => Some(action_count - 1),
                        (Some(index), KeyCode::Tab) if index + 1 < action_count => Some(index + 1),
                        (Some(index), KeyCode::BackTab) if index > 0 => Some(index - 1),
                        _ => None,
                    };
                    state.focused_action = next;
                    return Ok(TuiAction::Continue);
                }
                KeyCode::Left | KeyCode::Up if state.focused_action.is_some() => {
                    state.focused_action = Some((state.focused_action.unwrap() + action_count - 1) % action_count);
                    return Ok(TuiAction::Continue);
                }
                KeyCode::Right | KeyCode::Down if state.focused_action.is_some() => {
                    state.focused_action = Some((state.focused_action.unwrap() + 1) % action_count);
                    return Ok(TuiAction::Continue);
                }
                KeyCode::Enter if state.focused_action.is_some() => {
                    let code = actions.nth(state.focused_action.take().unwrap()).expect("Focused action exists").1;
                    return handle_key(state, KeyEvent::new(code, KeyModifiers::NONE), registry);
                }
                _ => {},
            }
        }
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
            {
                match key.code {
                    KeyCode::Esc if !state.editor_active => { browser.typing = false; },
                    KeyCode::Esc => { state.close(); return Ok(TuiAction::Continue); }
                    KeyCode::Tab | KeyCode::BackTab => browser.typing = !browser.typing,
                    KeyCode::Char('/') if !browser.typing => browser.typing = true,
                    KeyCode::Char('w') if !browser.typing => browser.previewing = !browser.previewing,
                    KeyCode::Char('l') if !browser.typing => {
                        state.dialog = Dialog::Browser(browser); state.browse_saved(registry)?; return Ok(TuiAction::Continue);
                    }
                    KeyCode::Char('q') if !browser.typing => {
                        state.dialog = Dialog::Browser(browser); return state.request_transition(Transition::Quit, registry);
                    }
                    KeyCode::Char(' ') if !browser.typing => state.session.set_paused(!state.session.is_paused()),
                    KeyCode::Char('f') if !browser.typing => {
                        state.dialog = Dialog::Browser(browser); state.toggle_fullscreen(); return Ok(TuiAction::Continue);
                    }
                    KeyCode::Enter => {
                        if let Some(name) = browser.names.get(browser.selected).cloned() {
                            match browser.purpose {
                                BrowserPurpose::Replace => { state.open(Dialog::ConfirmReplace(name)); state.temporary = None; },
                                BrowserPurpose::Add => { state.add_instance(&name, registry)?; state.close(); state.focus = PaneFocus::Inspector; state.view = EditorView::Edit; },
                                BrowserPurpose::New => { state.dialog = Dialog::Browser(browser); return state.request_transition(Transition::New(name), registry); },
                            }
                            return Ok(TuiAction::Continue);
                        }
                    }
                    KeyCode::Down if !browser.names.is_empty() => { browser.selected = (browser.selected + 1) % browser.names.len(); browser.description_scroll = 0; }
                    KeyCode::Up if !browser.names.is_empty() => { browser.selected = (browser.selected + browser.names.len() - 1) % browser.names.len(); browser.description_scroll = 0; }
                    KeyCode::PageDown => browser.description_scroll = browser.description_scroll.saturating_add(1),
                    KeyCode::PageUp => browser.description_scroll = browser.description_scroll.saturating_sub(1),
                    _ if browser.typing => { if edit_text(&mut browser.search, &mut browser.cursor, key, None) { browser.selected = 0; browser.description_scroll = 0; } }
                    _ => {},
                }
            }
            state.dialog = Dialog::Browser(browser); state.refresh_browser(registry)?;
        }
        Dialog::Saved(mut browser) => {
            let previous_selection = browser.selected;
            match key.code {
                KeyCode::Esc => { state.close(); return Ok(TuiAction::Continue); }
                KeyCode::Char('w') => browser.previewing = !browser.previewing,
                KeyCode::Down if !browser.entries.is_empty() => browser.selected = (browser.selected + 1) % browser.entries.len(),
                KeyCode::Up if !browser.entries.is_empty() => browser.selected = (browser.selected + browser.entries.len() - 1) % browser.entries.len(),
                KeyCode::Enter | KeyCode::Char('p' | 'f' | 'c') if key.code == KeyCode::Enter || browser.needs_save => {
                    if let Some(entry) = browser.entries.get(browser.selected) {
                        if state.temporary.is_some() {
                            let transition = Transition::Open(entry.path.clone(), entry.name.clone());
                            state.dialog = Dialog::Saved(browser);
                            return state.request_transition(transition, registry);
                        }
                    }
                }
                KeyCode::Char('p') | KeyCode::Char('f') if state.temporary.is_some() => {
                    state.dialog = Dialog::Saved(browser); state.toggle_fullscreen(); return Ok(TuiAction::Continue);
                }
                KeyCode::Char(' ') => state.session.set_paused(!state.session.is_paused()),
                KeyCode::Char('c') if state.temporary.is_some() => {
                    let entry = &browser.entries[browser.selected];
                    let command = format!("ascii-animation run --config {}", shell_quote(&absolute_path(&entry.path)?.to_string_lossy()));
                    state.dialog = Dialog::Saved(browser);
                    state.overlay(Dialog::Export { scroll: 0, choice: 0, command: Some(command) });
                    return Ok(TuiAction::Continue);
                }
                KeyCode::Char('n') => { state.close(); state.browse_presets(BrowserPurpose::New, registry)?; return Ok(TuiAction::Continue); }
                KeyCode::Char('r') => {
                    state.close(); state.browse_saved(registry)?; return Ok(TuiAction::Continue);
                }
                KeyCode::Char('e') => {
                    let error = match (&browser.directory_error, &browser.error) {
                        (Some(directory), Some(entry)) => format!("{directory}\n\n{entry}"),
                        (Some(error), None) | (None, Some(error)) => error.clone(),
                        (None, None) => "Selected Saved Scene can be opened. r retries reading; Esc returns.".into(),
                    };
                    state.dialog = Dialog::Saved(browser);
                    state.overlay(Dialog::ReadError { error, scroll: 0 });
                    return Ok(TuiAction::Continue);
                }
                _ => {},
            }
            let changed = browser.selected != previous_selection;
            state.dialog = Dialog::Saved(browser);
            if changed { state.refresh_saved(registry); }
        }
        Dialog::Name { mut text, mut cursor, mut error, after } => {
            match key.code {
                KeyCode::Esc => state.close(),
                KeyCode::Enter => {
                    match validate_scene_name(&text) {
                        Err(message) => { error = Some(message); state.dialog = Dialog::Name { text, cursor, error, after }; },
                        Ok(name) => {
                            let path = absolute_path(&state.library_path().join(format!("{name}.toml")))?;
                            return state.persist(SaveRequest { path, name, overwrite: false, after }, registry);
                        }
                    }
                }
                _ => { if edit_text(&mut text, &mut cursor, key, None) { error = None; } state.dialog = Dialog::Name { text, cursor, error, after }; }
            }
        }
        Dialog::Overwrite(mut request) => match key.code {
            KeyCode::Enter | KeyCode::Char('y') => { request.overwrite = true; return state.persist(request, registry); },
            KeyCode::Esc | KeyCode::Char('n') => state.close(),
            _ => state.dialog = Dialog::Overwrite(request),
        },
        Dialog::SaveError { request, error, mut scroll } => {
            match key.code {
                KeyCode::Enter | KeyCode::Char('r') => return state.persist(request, registry),
                KeyCode::Esc => state.close(),
                KeyCode::Down | KeyCode::PageDown => { scroll = scroll.saturating_add(3); state.dialog = Dialog::SaveError { request, error, scroll }; },
                KeyCode::Up | KeyCode::PageUp => { scroll = scroll.saturating_sub(3); state.dialog = Dialog::SaveError { request, error, scroll }; },
                _ => state.dialog = Dialog::SaveError { request, error, scroll },
            }
        }
        Dialog::ConfirmDelete => {
            match key.code {
                KeyCode::Enter | KeyCode::Char('y') => { state.remove_selected_instance(registry)?; state.close(); }
                KeyCode::Esc | KeyCode::Char('n') => state.close(),
                _ => state.dialog = Dialog::ConfirmDelete,
            }
        }
        Dialog::Guard { mut choice, transition } => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('c') => { state.close(); return Ok(TuiAction::Continue); },
                KeyCode::Left | KeyCode::Up | KeyCode::BackTab => choice = (choice + 2) % 3,
                KeyCode::Right | KeyCode::Down | KeyCode::Tab => choice = (choice + 1) % 3,
                KeyCode::Char('s') => choice = 0,
                KeyCode::Char('d') => choice = 1,
                _ => {},
            }
            if matches!(key.code, KeyCode::Enter | KeyCode::Char('s') | KeyCode::Char('d')) {
                match choice {
                    0 => {
                        state.dialog = Dialog::Guard { choice, transition: transition.clone() };
                        return state.request_save(false, AfterSave::Transition(transition), registry);
                    }
                    1 => return state.transition(transition, registry),
                    _ => state.close(),
                }
            } else { state.dialog = Dialog::Guard { choice, transition }; }
        }
        Dialog::Export { mut scroll, mut choice, command } => {
            if key.code == KeyCode::Esc { state.close(); return Ok(TuiAction::Continue); }
            match key.code {
                KeyCode::Down => scroll = scroll.saturating_add(1), KeyCode::Up => scroll = scroll.saturating_sub(1),
                KeyCode::PageDown => scroll = scroll.saturating_add(8), KeyCode::PageUp => scroll = scroll.saturating_sub(8),
                KeyCode::Home => scroll = 0,
                KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => choice = 1 - choice,
                KeyCode::Enter if choice == 1 => { state.close(); return Ok(TuiAction::Continue); },
                KeyCode::Enter | KeyCode::Char('c') => {
                    if let Some(saved_command) = &command {
                        let copied = saved_command.clone();
                        state.dialog = Dialog::Export { scroll, choice, command };
                        return Ok(TuiAction::CopyCommand(copied));
                    }
                    if state.is_dirty() {
                        state.dialog = Dialog::Export { scroll, choice, command };
                        return state.request_save(false, AfterSave::Copy, registry);
                    }
                    let copied = state.export_command();
                    state.dialog = Dialog::Export { scroll, choice, command: Some(copied.clone()) };
                    return Ok(TuiAction::CopyCommand(copied));
                }
                _ => {},
            }
            state.dialog = Dialog::Export { scroll, choice, command };
        }
        Dialog::Help(mut scroll) => {
            let mut actions = available_actions(state, true);
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => state.close(),
                KeyCode::Down | KeyCode::Tab => { scroll = scroll.saturating_add(1); state.dialog = Dialog::Help(scroll); },
                KeyCode::Up | KeyCode::BackTab => { scroll = scroll.saturating_sub(1); state.dialog = Dialog::Help(scroll); },
                KeyCode::PageDown => { scroll = scroll.saturating_add(5); state.dialog = Dialog::Help(scroll); },
                KeyCode::PageUp => { scroll = scroll.saturating_sub(5); state.dialog = Dialog::Help(scroll); },
                KeyCode::Enter => {
                    if let Some((_, code)) = actions.nth(scroll as usize) {
                        state.close();
                        state.focused_action = None;
                        return handle_key(state, KeyEvent::new(*code, KeyModifiers::NONE), registry);
                    }
                    state.dialog = Dialog::Help(scroll);
                }
                KeyCode::Char(ch) if actions.any(|(_, code)| *code == KeyCode::Char(ch)) => {
                    state.close(); state.focused_action = None; return handle_key(state, key, registry);
                }
                _ => state.dialog = Dialog::Help(scroll),
            }
        }
        Dialog::ReadError { error, mut scroll } => {
            match key.code {
                KeyCode::Esc => state.close(),
                KeyCode::Char('r') => {
                    state.close();
                    if matches!(state.dialog, Dialog::Saved(_)) { state.close(); }
                    state.browse_saved(registry)?;
                },
                KeyCode::Down | KeyCode::PageDown => { scroll = scroll.saturating_add(3); state.dialog = Dialog::ReadError { error, scroll }; },
                KeyCode::Up | KeyCode::PageUp => { scroll = scroll.saturating_sub(3); state.dialog = Dialog::ReadError { error, scroll }; },
                _ => state.dialog = Dialog::ReadError { error, scroll },
            }
        }
        Dialog::Recovery(mut scroll) => {
            match key.code {
                KeyCode::Char('r') => { *state = TuiState::load_from_path(&state.config_path, registry)?; }
                KeyCode::Enter | KeyCode::Char('d') => { state.close(); state.saved_name = None; state.status = Some("Opened unsaved default; original file remains unchanged".into()); }
                KeyCode::Char('q') => return Ok(TuiAction::Quit),
                KeyCode::Down | KeyCode::PageDown => { scroll = scroll.saturating_add(3); state.dialog = Dialog::Recovery(scroll); },
                KeyCode::Up | KeyCode::PageUp => { scroll = scroll.saturating_sub(3); state.dialog = Dialog::Recovery(scroll); },
                _ => state.dialog = Dialog::Recovery(scroll),
            }
        }
        Dialog::ConfirmReplace(name) => match key.code {
            KeyCode::Enter | KeyCode::Char('y') => { state.replace_selected(&name, registry)?; state.close(); },
            KeyCode::Esc | KeyCode::Char('n') => state.close(),
            _ => state.dialog = Dialog::ConfirmReplace(name),
        },
        Dialog::None => {
            match key.code {
                KeyCode::Char('q') => return state.request_transition(Transition::Quit, registry),
                KeyCode::Char(' ') => state.session.set_paused(!state.session.is_paused()),
                KeyCode::Char('f') => state.toggle_fullscreen(),
                KeyCode::Char('a') => state.open_browser(false, registry)?,
                KeyCode::Char('r') if state.focus == PaneFocus::Scene => state.open_browser(true, registry)?,
                KeyCode::Char('s') => return state.request_save(false, AfterSave::Stay, registry),
                KeyCode::Char('S') => return state.request_save(true, AfterSave::Stay, registry),
                KeyCode::Char('c') => state.open(Dialog::Export { scroll: 0, choice: 0, command: None }),
                KeyCode::Char('n') => state.browse_presets(BrowserPurpose::New, registry)?,
                KeyCode::Char('l') => state.browse_saved(registry)?,
                KeyCode::Char('v') => { state.focus = PaneFocus::Scene; state.view = EditorView::Scene; },
                KeyCode::Esc => { state.focus = PaneFocus::Preview; state.view = EditorView::Preview; },
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

fn handle_mouse(state: &mut TuiState, mouse: MouseEvent, registry: &PresetRegistry) -> Result<TuiAction> {
    if state.hit_targets.is_empty() { return Ok(TuiAction::Continue); }
    let target = state.hit_targets.iter().rev().find(|target| contains(target.area, mouse.column, mouse.row)).map(|target| target.action.clone());
    if !matches!(target, Some(HitAction::Key(KeyCode::F(1)))) && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left) | MouseEventKind::ScrollDown | MouseEventKind::ScrollUp) {
        state.focused_action = None;
    }
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => match target {
            Some(HitAction::Key(code)) => {
                if code != KeyCode::F(1) && code != KeyCode::Esc { state.focused_action = None; }
                return handle_key(state, KeyEvent::new(code, KeyModifiers::NONE), registry);
            }
            Some(HitAction::HelpAction(code)) => {
                state.close();
                state.focused_action = None;
                return handle_key(state, KeyEvent::new(code, KeyModifiers::NONE), registry);
            }
            Some(HitAction::Preset(index)) => {
                if let Dialog::Browser(browser) = &mut state.dialog { browser.selected = index; browser.typing = false; browser.description_scroll = 0; }
                state.refresh_browser(registry)?;
            }
            Some(HitAction::Saved(index)) => {
                let changed = if let Dialog::Saved(browser) = &mut state.dialog {
                    let changed = browser.selected != index; browser.selected = index; changed
                } else { false };
                if changed { state.refresh_saved(registry); }
            }
            Some(HitAction::Instance(index)) => {
                state.selected_instance = index; state.selected_option = 0; state.focus = PaneFocus::Scene; state.view = EditorView::Scene; state.sync_selected_options(registry)?;
            }
            Some(HitAction::Field(index)) => {
                state.selected_option = index; state.focus = PaneFocus::Inspector; state.view = EditorView::Edit;
            }
            Some(HitAction::Focus(focus)) => { state.focus = focus; state.view = EditorView::Preview; },
            None => {},
        }
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            if mouse.row >= state.terminal_size.1.saturating_sub(3) { return Ok(TuiAction::Continue); }
            let layout = tui_layout(Rect::new(0, 0, state.terminal_size.0, state.terminal_size.1));
            if matches!(state.dialog, Dialog::Browser(_) | Dialog::Saved(_) | Dialog::None)
                && !contains(layout.options, mouse.column, mouse.row) { return Ok(TuiAction::Continue); }
            if state.fullscreen && matches!(state.dialog, Dialog::None) { return Ok(TuiAction::Continue); }
            if editor_layout(state.terminal_size.0, state.terminal_size.1) == EditorLayout::Small {
                let options_visible = match &state.dialog {
                    Dialog::Browser(browser) => !browser.previewing,
                    Dialog::Saved(browser) => !browser.previewing,
                    Dialog::None => state.focus == PaneFocus::Scene || state.view == EditorView::Edit,
                    _ => true,
                };
                if !options_visible { return Ok(TuiAction::Continue); }
            }
            let down = mouse.kind == MouseEventKind::ScrollDown;
            let code = match &state.dialog {
                Dialog::Editor(_) | Dialog::Name { .. } | Dialog::Overwrite(_) | Dialog::Guard { .. } | Dialog::ConfirmDelete | Dialog::ConfirmReplace(_) => return Ok(TuiAction::Continue),
                Dialog::None => {
                    if state.focus != PaneFocus::Scene { state.focus = PaneFocus::Inspector; state.view = EditorView::Edit; }
                    if down { KeyCode::Down } else { KeyCode::Up }
                },
                Dialog::Browser(_) if !matches!(target, Some(HitAction::Preset(_))) => if down { KeyCode::PageDown } else { KeyCode::PageUp },
                _ => if down { KeyCode::Down } else { KeyCode::Up },
            };
            return handle_key(state, KeyEvent::new(code, KeyModifiers::NONE), registry);
        }
        _ => {},
    }
    Ok(TuiAction::Continue)
}
fn contains(area: Rect, x: u16, y: u16) -> bool { x >= area.x && y >= area.y && x < area.right() && y < area.bottom() }
fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() { Ok(path.to_path_buf()) }
    else { std::env::current_dir().map(|cwd| cwd.join(path)).map_err(terminal_error) }
}
fn validate_scene_name(text: &str) -> std::result::Result<String, String> {
    let name = text.trim();
    if name.is_empty() || name.len() > 80 || !name.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_')) {
        Err("Use 1–80 ASCII letters, digits, spaces, - or _. Paths, dots and shell syntax are not allowed.".into())
    } else { Ok(name.into()) }
}
fn key_label(code: KeyCode) -> String {
    match code { KeyCode::Char(' ') => "Space".into(), KeyCode::Char(ch) => ch.to_string(), KeyCode::Enter => "Enter".into(), KeyCode::Esc => "Esc".into(),
        KeyCode::Left => "Left".into(), KeyCode::Right => "Right".into(), KeyCode::Tab => "Tab".into(), KeyCode::F(1) => "F1".into(), other => format!("{other:?}") }
}
fn available_actions(state: &TuiState, underlay: bool) -> impl Iterator<Item = &'static (&'static str, KeyCode)> + Clone + 'static {
    let dialog = if underlay { state.underlays.last().map(|underlay| &underlay.dialog).unwrap_or(&state.dialog) } else { &state.dialog };
    let empty: &'static [(&'static str, KeyCode)] = &[];
    let (first, second, third): (&'static [(&'static str, KeyCode)], &'static [(&'static str, KeyCode)], &'static [(&'static str, KeyCode)]) = match dialog {
        Dialog::Browser(browser) if browser.typing => (&[("Edit", KeyCode::Enter), ("Stop typing", KeyCode::Tab), ("Cancel", KeyCode::Esc)], empty, empty),
        Dialog::Browser(_) => (&[("Edit", KeyCode::Enter), ("Saved Scenes", KeyCode::Char('l')), ("Search", KeyCode::Char('/')), ("Back", KeyCode::Esc), ("Pause/Resume", KeyCode::Char(' ')), ("Fullscreen", KeyCode::Char('f')), ("Quit", KeyCode::Char('q')),
            ("Preview/List", KeyCode::Char('w')), ("Type search", KeyCode::Tab)], empty, empty),
        Dialog::Saved(browser) if browser.needs_save => (&[("Open/Edit to Save", KeyCode::Enter), ("Pause/Resume", KeyCode::Char(' ')), ("New", KeyCode::Char('n')), ("Back", KeyCode::Esc), ("Retry read", KeyCode::Char('r')), ("Details", KeyCode::Char('e')), ("Preview/List", KeyCode::Char('w'))], empty, empty),
        Dialog::Saved(_) => (&[("Open/Edit", KeyCode::Enter), ("Play", KeyCode::Char('p')), ("Pause/Resume", KeyCode::Char(' ')), ("Copy Command", KeyCode::Char('c')), ("New", KeyCode::Char('n')), ("Back", KeyCode::Esc), ("Retry read", KeyCode::Char('r')), ("Error details", KeyCode::Char('e')), ("Preview/List", KeyCode::Char('w'))], empty, empty),
        Dialog::Name { .. } => (&[("Save", KeyCode::Enter), ("Cancel", KeyCode::Esc)], empty, empty),
        Dialog::Overwrite(_) => (&[("Overwrite", KeyCode::Enter), ("Cancel", KeyCode::Esc)], empty, empty),
        Dialog::SaveError { .. } => (&[("Retry", KeyCode::Char('r')), ("Cancel", KeyCode::Esc)], empty, empty),
        Dialog::ReadError { .. } => (&[("Retry", KeyCode::Char('r')), ("Back", KeyCode::Esc)], empty, empty),
        Dialog::Guard { .. } => (&[("Save", KeyCode::Char('s')), ("Discard", KeyCode::Char('d')), ("Cancel", KeyCode::Esc)], empty, empty),
        Dialog::Editor(_) => (&[("Commit", KeyCode::Enter), ("Cancel", KeyCode::Esc), ("Previous", KeyCode::Left), ("Next", KeyCode::Right)], empty, empty),
        Dialog::ConfirmDelete | Dialog::ConfirmReplace(_) => (&[("Confirm", KeyCode::Enter), ("Cancel", KeyCode::Esc)], empty, empty),
        Dialog::Export { command, .. } => (if command.is_none() && state.is_dirty() {
            &[("Save and Copy", KeyCode::Char('c')), ("Back", KeyCode::Esc)]
        } else { &[("Copy", KeyCode::Char('c')), ("Back", KeyCode::Esc)] }, empty, empty),
        Dialog::Recovery(_) => (&[("Reload", KeyCode::Char('r')), ("Unsaved defaults", KeyCode::Char('d')), ("Quit", KeyCode::Char('q'))], empty, empty),
        Dialog::Help(_) => (&[("Back", KeyCode::Esc)], empty, empty),
        Dialog::None if state.fullscreen => (if state.is_paused() {
            &[("Resume", KeyCode::Char(' ')), ("Back", KeyCode::Esc), ("Quit", KeyCode::Char('q'))]
        } else { &[("Pause", KeyCode::Char(' ')), ("Back", KeyCode::Esc), ("Quit", KeyCode::Char('q'))] }, empty, empty),
        Dialog::None => (
            &[("Save", KeyCode::Char('s')), ("Save As", KeyCode::Char('S'))],
            if state.is_dirty() { &[("Save and Copy", KeyCode::Char('c'))] } else { &[("Copy Command", KeyCode::Char('c'))] },
            &[("New", KeyCode::Char('n')), ("Saved Scenes", KeyCode::Char('l')), ("Animations", KeyCode::Char('v')),
                ("Pause/Resume", KeyCode::Char(' ')), ("Fullscreen", KeyCode::Char('f')), ("Quit", KeyCode::Char('q')), ("Focus pane", KeyCode::Tab)],
        ),
    };
    let context: &'static [(&'static str, KeyCode)] = if matches!(dialog, Dialog::None) && !state.fullscreen {
        match state.focus {
            PaneFocus::Inspector => &[("Decrease", KeyCode::Left), ("Increase", KeyCode::Right), ("Edit value", KeyCode::Enter)],
            PaneFocus::Scene => &[("Add", KeyCode::Char('a')), ("Replace", KeyCode::Char('r')), ("Delete", KeyCode::Char('d')), ("Move up", KeyCode::Char('[')), ("Move down", KeyCode::Char(']')), ("Edit instance", KeyCode::Enter)],
            PaneFocus::Preview => empty,
        }
    } else { empty };
    [first, second, third, context].into_iter().flat_map(|slice| slice.iter())
}
fn draw_actions(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    let actions = available_actions(state, false);
    let typing = matches!(&state.dialog, Dialog::Name { .. } | Dialog::Editor(Draft { value: OptionValue::Text(_), .. }))
        || matches!(&state.dialog, Dialog::Browser(browser) if browser.typing);
    let help = if matches!(state.dialog, Dialog::Help(_)) { ("Back", KeyCode::Esc) } else { ("Help/Actions", KeyCode::F(1)) };
    let help_text = format!("[{} {}]", key_label(help.1), help.0);
    frame.render_widget(Paragraph::new(help_text.as_str()).style(Style::default().fg(AMBER)), Rect::new(area.x, area.y, area.width, 1));
    state.hit_targets.push(HitTarget { area: Rect::new(area.x, area.y, help_text.len().min(area.width as usize) as u16, 1), action: HitAction::Key(help.1) });
    let guidance = if typing { "Typing text; Esc cancels. F1 Help." } else if matches!(state.dialog, Dialog::None) && state.focus == PaneFocus::Inspector { "Up/Down select; arrows adjust; Shift fast" } else { state.copy_status.as_deref().or(state.status.as_deref()).unwrap_or("Tab focus; Up/Down select; Enter act") };
    let mut x = area.x; let mut y = area.y + 1;
    let focused = match &state.dialog { Dialog::Export { choice, .. } => Some(*choice), _ => state.focused_action };
    let start = focused.map(|index| index.saturating_sub(1)).unwrap_or(0);
    for (index, (label, key)) in actions.enumerate().skip(start) {
        let text = format!("[{}{} {}]", if focused == Some(index) { ">" } else { "" }, key_label(*key), label);
        let width = text.len() as u16;
        if width > area.width { continue; }
        if x + width > area.right() { x = area.x; y += 1; }
        if y >= area.bottom() { break; }
        let target = Rect::new(x, y, width, 1);
        frame.render_widget(Paragraph::new(text).style(Style::default().fg(MUTED)), target);
        state.hit_targets.push(HitTarget { area: target, action: HitAction::Key(*key) });
        x += width + 1;
    }
    if y == area.y + 1 { frame.render_widget(Paragraph::new(guidance), Rect::new(area.x, area.y + 2, area.width, 1)); }
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
    Field { name: name.into(), label: label.into(), kind, group: OptionGroup::Layout, help: help.into(), rebuilds: false }
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
    if let Err(err) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) { return restore_tui_setup(true).and(Err(terminal_error(err))); }
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
        let now = Instant::now(); handle_tui_event(state, TuiEvent::Advance(now.duration_since(previous)), registry)?; previous = now;
        terminal.draw(|frame| render_tui(frame, registry, state)).map_err(terminal_error)?;
        let interval = Duration::from_secs_f64(1.0 / f64::from(state.active_preview().scene().frame_rate.max(1)));
        if event::poll(interval).map_err(terminal_error)? {
            let input = event::read().map_err(terminal_error)?;
            let accepted_at = Instant::now();
            handle_tui_event(state, TuiEvent::Advance(accepted_at.duration_since(previous)), registry)?;
            match handle_tui_event(state, input, registry) {
                Ok(TuiAction::Quit) => return Ok(()),
                Ok(TuiAction::CopyCommand(command)) => { handle_tui_event(state, TuiEvent::Clipboard(copy_to_clipboard(&command).map_err(|err| err.to_string())), registry)?; },
                Ok(TuiAction::Continue) => {},
                Err(err) => state.status = Some(err.to_string()),
            }
            previous = Instant::now();
        }
    }
}

fn panel(title: String, focused: bool) -> Block<'static> {
    Block::default().title(title).borders(Borders::ALL).border_style(Style::default().fg(if focused { AMBER } else { MUTED }))
}
/// Draws the actual TUI and records only visible, current mouse targets.
pub fn render_tui(frame: &mut Frame<'_>, registry: &PresetRegistry, state: &mut TuiState) {
    let area = frame.area();
    state.resize(area.width, area.height);
    state.hit_targets.clear();
    frame.render_widget(Block::default().style(Style::default().bg(GRAPHITE).fg(PAPER)), area);
    if editor_layout(area.width, area.height) == EditorLayout::Tiny {
        let help = matches!(state.dialog, Dialog::Help(_));
        let text = if help { "Resize: 36x10\nEditing waits for resize.\nWork retained." } else { "Resize: 36x10\nWork retained." };
        let body = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), body);
        if area.width >= 9 && area.height > 1 {
            let label = if help { "Esc Back" } else { "F1 Help" };
            let target = Rect::new(area.x, area.y + area.height - 1, label.len() as u16, 1);
            frame.render_widget(Paragraph::new(label).style(Style::default().fg(AMBER)), target);
            state.hit_targets.push(HitTarget { area: target, action: HitAction::Key(if help { KeyCode::Esc } else { KeyCode::F(1) }) });
        }
        return;
    }
    let fullscreen = state.fullscreen && matches!(state.dialog, Dialog::None);
    let layout = tui_layout(area);
    let browsing = matches!(state.dialog, Dialog::Browser(_) | Dialog::Saved(_));
    let compact = editor_layout(area.width, area.height) == EditorLayout::Small;
    let browser_preview = match &state.dialog { Dialog::Browser(browser) => browser.previewing, Dialog::Saved(browser) => browser.previewing, _ => false };
    let title = if fullscreen { "Fullscreen playback".into() } else {
        format!("{} | {} | {}", if state.is_dirty() { "unsaved" } else { "saved" }, if state.is_paused() { "paused" } else { "running" }, state.saved_name.as_deref().unwrap_or("New Scene"))
    };
    frame.render_widget(Paragraph::new(title).style(Style::default().fg(AMBER)), Rect::new(area.x, area.y, area.width, 1));
    let preview_area = if fullscreen { Rect::new(area.x, area.y + 1, area.width, area.height.saturating_sub(4)) } else { layout.preview };
    let (width, height) = crate::runtime::viewport_for_canvas(state.active_preview().canvas_dimensions().0, area.width, area.height);
    let preview = state.preview_text(width, height);
    if fullscreen || !compact || (browsing && browser_preview) || (!browsing && state.view == EditorView::Preview) {
        if fullscreen {
            frame.render_widget(Paragraph::new(preview), preview_area);
        } else {
            let cropped = width > preview_area.width.saturating_sub(2) || height > preview_area.height.saturating_sub(2);
            frame.render_widget(Paragraph::new(preview).block(panel(format!("{}Live preview{}", if state.focus == PaneFocus::Preview { "> " } else { "" }, if cropped { " [Canvas cropped]" } else { "" }), state.focus == PaneFocus::Preview)), preview_area);
        }
        if !browsing && matches!(state.dialog, Dialog::None) && !fullscreen {
            state.hit_targets.push(HitTarget { area: preview_area, action: HitAction::Focus(PaneFocus::Preview) });
        }
    }
    if browsing && (!compact || !browser_preview) { draw_browser(frame, state, registry, layout.options); }
    else if matches!(state.dialog, Dialog::None) && !fullscreen {
        if state.focus == PaneFocus::Scene { draw_scene(frame, state, layout.options); }
        else if !compact || state.view == EditorView::Edit { draw_inspector(frame, state, layout.options); }
    } else if !fullscreen && !browsing { draw_dialog(frame, state, registry, area); }
    let footer = Rect::new(area.x, area.y + area.height - 3, area.width, 3);
    draw_actions(frame, state, footer);
}
fn keep_visible(scroll: &mut usize, selected: usize, count: usize) {
    let count = count.max(1);
    if selected < *scroll { *scroll = selected; }
    else if selected >= scroll.saturating_add(count) { *scroll = selected + 1 - count; }
}
fn draw_scene(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    let block = panel(format!("> Animations ({})", state.scene.instances.len()), true);
    let inner = block.inner(area);
    keep_visible(&mut state.scene_scroll, state.selected_instance, inner.height as usize);
    frame.render_widget(block, area);
    for (row, (index, instance)) in state.scene.instances.iter().enumerate().skip(state.scene_scroll).take(inner.height as usize).enumerate() {
        let target = Rect::new(inner.x, inner.y + row as u16, inner.width, 1);
        frame.render_widget(Paragraph::new(format!("{} [{}] {} ({})", if index == state.selected_instance { ">" } else { " " }, if instance.enabled { "on" } else { "off" }, instance.id, instance.preset)), target);
        state.hit_targets.push(HitTarget { area: target, action: HitAction::Instance(index) });
    }
}
fn draw_inspector(frame: &mut Frame<'_>, state: &mut TuiState, area: Rect) {
    let mut lines = Vec::new(); let mut indices = Vec::new(); let mut group = None; let mut selected_row = 0;
    for (index, field) in state.fields.iter().enumerate() {
        if field.group == OptionGroup::Layout && state.focus != PaneFocus::Inspector { continue; }
        if group != Some(group_index(field.group)) {
            lines.push(Line::styled(group_label(field.group), Style::default().fg(MUTED)));
            indices.push(None); group = Some(group_index(field.group));
        }
        if index == state.selected_option { selected_row = lines.len(); }
        let value = state.field_value(&field.name).map(|v| format_tui_option_value(&v)).unwrap_or_default();
        lines.push(Line::styled(format!("{} {}: {}{}", if index == state.selected_option { ">" } else { " " }, field.label, value, if field.rebuilds { " [restart]" } else { "" }),
            Style::default().fg(if index == state.selected_option { AMBER } else { PAPER })));
        indices.push(Some(index));
    }
    let help_height = if area.height >= 10 { 3 } else { 1 };
    let content = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(help_height));
    let block = panel(format!("{}Options: {}", if state.focus == PaneFocus::Inspector { "> " } else { "" }, state.selected_instance().preset), state.focus == PaneFocus::Inspector);
    let inner = block.inner(content);
    keep_visible(&mut state.inspector_scroll, selected_row, inner.height as usize);
    frame.render_widget(Paragraph::new(lines).scroll((state.inspector_scroll.min(u16::MAX as usize) as u16, 0)).block(block), content);
    for (row, index) in indices.iter().skip(state.inspector_scroll).take(inner.height as usize).enumerate() {
        if let Some(index) = index {
            state.hit_targets.push(HitTarget { area: Rect::new(inner.x, inner.y + row as u16, inner.width, 1), action: HitAction::Field(*index) });
        }
    }
    if let Some(field) = state.fields.get(state.selected_option) {
        frame.render_widget(Paragraph::new(format!("{}: {}\n{}", field.label, state.field_value(&field.name).map(|v| format_tui_option_value(&v)).unwrap_or_default(), field.help)).style(Style::default().fg(MUTED)).wrap(Wrap { trim: false }),
            Rect::new(area.x + 1, content.y + content.height, area.width.saturating_sub(2), help_height));
    }
}
fn modal_area(area: Rect, width: u16, height: u16) -> Rect {
    let body = Rect::new(area.x, area.y.saturating_add(1), area.width, area.height.saturating_sub(4));
    let width = width.min(body.width); let height = height.min(body.height.max(1));
    Rect::new(body.x + body.width.saturating_sub(width) / 2, body.y + body.height.saturating_sub(height) / 2, width, height)
}
fn draw_browser(frame: &mut Frame<'_>, state: &mut TuiState, registry: &PresetRegistry, area: Rect) {
    let block = panel(match &state.dialog {
        Dialog::Browser(browser) => match browser.purpose { BrowserPurpose::New => "Presets", BrowserPurpose::Add => "Add animation", BrowserPurpose::Replace => "Replace Preset" },
        _ => "Saved Scenes",
    }.into(), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    match &mut state.dialog {
        Dialog::Browser(browser) => {
            frame.render_widget(Paragraph::new(format!("{}Find: {}", if browser.typing { "> " } else { "  " }, cursor_window(&browser.search, browser.cursor, inner.width.saturating_sub(8) as usize))), Rect::new(inner.x, inner.y, inner.width, 1));
            let details = if inner.height >= 8 { 3 } else { 1 };
            let rows = inner.height.saturating_sub(1 + details) as usize;
            let start = browser.selected.saturating_sub(rows.saturating_sub(1));
            for (row, (index, name)) in browser.names.iter().enumerate().skip(start).take(rows).enumerate() {
                let target = Rect::new(inner.x, inner.y + 1 + row as u16, inner.width, 1);
                frame.render_widget(Paragraph::new(format!("{} {}", if index == browser.selected { ">" } else { " " }, registry.get(name).expect("registered Preset").label())).style(Style::default().fg(if index == browser.selected { AMBER } else { PAPER })), target);
                state.hit_targets.push(HitTarget { area: target, action: HitAction::Preset(index) });
            }
            let description = browser.names.get(browser.selected).map(|name| registry.get(name).expect("registered Preset").description().to_string()).unwrap_or_else(|| "No matches. / searches; Esc returns.".into());
            frame.render_widget(Paragraph::new(description).wrap(Wrap { trim: false }).scroll((browser.description_scroll, 0)), Rect::new(inner.x, inner.y + 1 + rows as u16, inner.width, details));
        }
        Dialog::Saved(browser) => {
            let notice = browser.directory_error.as_deref().or(browser.error.as_deref());
            if browser.entries.is_empty() {
                frame.render_widget(Paragraph::new(notice.unwrap_or("No Saved Scenes yet. New chooses a Preset; Edit and Save name your first Scene.")).wrap(Wrap { trim: false }), inner);
            } else {
                let error_height = if notice.is_some() { inner.height.min(3) } else { 0 };
                let rows = inner.height.saturating_sub(error_height) as usize;
                keep_visible(&mut browser.scroll, browser.selected, rows);
                for (row, (index, entry)) in browser.entries.iter().enumerate().skip(browser.scroll).take(rows).enumerate() {
                    let target = Rect::new(inner.x, inner.y + row as u16, inner.width, 1);
                    frame.render_widget(Paragraph::new(format!("{} {}", if index == browser.selected { ">" } else { " " }, entry.name)).style(Style::default().fg(if index == browser.selected { AMBER } else { PAPER })), target);
                    state.hit_targets.push(HitTarget { area: target, action: HitAction::Saved(index) });
                }
                if let Some(notice) = notice { frame.render_widget(Paragraph::new(notice).wrap(Wrap { trim: false }), Rect::new(inner.x, inner.y + rows as u16, inner.width, error_height)); }
            }
        }
        _ => {},
    }
}

fn draw_dialog(frame: &mut Frame<'_>, state: &mut TuiState, _registry: &PresetRegistry, area: Rect) {
    let modal = modal_area(area, 82, 18);
    frame.render_widget(Clear, modal);
    let help_actions = matches!(state.dialog, Dialog::Help(_)).then(|| available_actions(state, true));
    let (title, text, scroll) = match &state.dialog {
        Dialog::Name { text, cursor, error, .. } => ("Name Saved Scene".into(), format!("{}\n{}\nTyping owns shortcuts. Enter saves; Esc cancels; F1 Help.", cursor_window(text, *cursor, modal.width.saturating_sub(4) as usize), error.as_deref().unwrap_or("1–80 ASCII letters, digits, spaces, - or _. Save preserves configuration, not runtime history.")), 0),
        Dialog::Overwrite(request) => ("Overwrite Saved Scene?".into(), format!("Replace {}?\n{}\nExisting configuration is lost only after a successful save. Enter/y confirms; Esc/n cancels.", request.name, request.path.display()), 0),
        Dialog::SaveError { request, error, scroll } => ("Cannot save Scene".into(), format!("{}\n{}\nUnsaved changes and previous saved target retained.", request.path.display(), error), *scroll),
        Dialog::Editor(draft) => {
            let field = state.fields.iter().find(|f| f.name == draft.name).expect("draft field");
            let value = match &draft.value { OptionValue::Text(text) => cursor_window(text, draft.cursor, modal.width.saturating_sub(4) as usize), value => format_tui_option_value(value) };
            (format!("Edit {}", field.label), format!("{value}\n{}\nEnter commits; Esc cancels; F1 Help.\nText: arrows/Home/End; Backspace/Delete.", draft.error.as_deref().unwrap_or(&field.help)), 0)
        }
        Dialog::ConfirmDelete => ("Delete animation instance?".into(), format!("Remove {} and all configured options?\nEnter/y deletes; Esc/n cancels.", state.selected_instance().id), 0),
        Dialog::ConfirmReplace(name) => ("Replace configured Preset?".into(), format!("Replace {} with {name}? Old Preset options are lost.\nPlacement, Layer and Z-index remain.\nEnter/y confirms; Esc/n cancels.", state.selected_instance().id), 0),
        Dialog::Guard { choice, transition } => ("Unsaved Scene".into(), format!("Save before {}?\n{}\nSave / Discard / Cancel. Escape returns without changing work.", match transition { Transition::Quit => "quitting", Transition::New(_) => "starting a new Scene", Transition::Open(..) => "opening another Scene" }, selected_buttons(&["Save", "Discard", "Cancel"], *choice)), 0),
        Dialog::Export { scroll, command, .. } => {
            let generated;
            let text = match command.as_deref() {
                Some(command) => command,
                None if state.is_dirty() => "Save and Copy creates a saved target before copying.",
                None => { generated = state.export_command(); &generated },
            };
            ("Copy playback command".into(), format!("{text}\n\nUses a local Scene file on this machine. Later Save updates the same target; another Scene has its own file.\n{}\n{}\nUp/Down/PgUp/PgDn scroll; Esc returns.",
                state.copy_status.as_deref().unwrap_or(""), state.status.as_deref().unwrap_or("")), *scroll)
        }
        Dialog::Help(scroll) => {
            let actions = help_actions.as_ref().expect("Help actions");
            let mut text = String::new();
            for (index, (label, code)) in actions.clone().enumerate() {
                if index > 0 { text.push('\n'); }
                let _ = write!(text, "{} {}: {label}", if index == *scroll as usize { ">" } else { " " }, key_label(*code));
            }
            let context = state.underlays.last().map(|underlay| &underlay.dialog).unwrap_or(&state.dialog);
            let details = match context {
                Dialog::Name { text, error, .. } => format!("Name draft: {text}\n{}\nUse 1–80 ASCII letters, digits, spaces, - or _. Paths and shell syntax are not allowed.", error.as_deref().unwrap_or("Enter saves; Escape cancels without writing.")),
                Dialog::Editor(draft) => {
                    let field = state.fields.iter().find(|field| field.name == draft.name).expect("draft field");
                    format!("{}: {}\n{}\n{}", field.label, format_tui_option_value(&draft.value), field.help, draft.error.as_deref().unwrap_or("Draft only. Enter commits; Escape cancels."))
                }
                Dialog::SaveError { request, error, .. } => format!("{}\n{error}", request.path.display()),
                Dialog::ReadError { error, .. } => error.clone(),
                Dialog::Saved(browser) => match (&browser.directory_error, &browser.error) {
                    (Some(directory), Some(entry)) => format!("{directory}\n\n{entry}"),
                    (Some(error), None) | (None, Some(error)) => error.clone(),
                    (None, None) => "Saved Scenes preserve complete configuration. Open/Edit replaces work only after the unsaved-work guard.".into(),
                },
                Dialog::None if state.focus == PaneFocus::Inspector => {
                    state.fields.get(state.selected_option).map(|field| format!("{}: {}\n{}\nLeft/Right adjusts; Shift adjusts numeric values faster. Enter edits text and choices.", field.label, state.field_value(&field.name).map(|value| format_tui_option_value(&value)).unwrap_or_default(), field.help)).unwrap_or_default()
                }
                _ => String::new(),
            };
            if !details.is_empty() { text.push_str("\n\n"); text.push_str(&details); }
            text.push_str("\n\nTab / Shift+Tab changes focus. Up/Down selects visible fields or entries. Left/Right adjusts; Shift adjusts faster. Enter edits text/choices; Esc cancels drafts. Search/name/text own printable keys. F1 opens Help without entering text.\nAnimations: a adds, r replaces, d deletes, [/] reorders. Placement, Layer and Z-index are distinct controls.\nPreview and direct playback share the Viewport. Saved Scenes preserve all instances and playback settings, not runtime history.\nUp/Down/PgUp/PgDn scroll. Esc returns.");
            ("Help / Actions".into(), text, *scroll)
        }
        Dialog::Recovery(scroll) => ("Cannot open saved Scene".into(), format!("Original file is unchanged.\nr reloads; d/Enter opens unsaved defaults; q quits.\n{}\n{}", state.config_path.display(), state.startup_error.as_deref().unwrap_or("Invalid Scene")), *scroll),
        Dialog::ReadError { error, scroll } => ("Saved Scene read error".into(), format!("{error}\nFile unchanged. r retries; Esc returns; F1 Help."), *scroll),
        _ => return,
    };
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }).scroll((scroll, 0)).block(panel(title, true)), modal);
    if matches!(state.dialog, Dialog::Help(_)) {
        let inner = panel(String::new(), true).inner(modal);
        let actions = help_actions.as_ref().expect("Help actions");
        for (row, (_, code)) in actions.clone().skip(scroll as usize).take(inner.height as usize).enumerate() {
            state.hit_targets.push(HitTarget { area: Rect::new(inner.x, inner.y + row as u16, inner.width, 1), action: HitAction::HelpAction(*code) });
        }
    }
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
    let restore = execute!(terminal.backend_mut(), Show, DisableMouseCapture, LeaveAlternateScreen).err();
    finish_tui_restore(restore, terminal::disable_raw_mode().err())
}
fn restore_tui_setup(entered: bool) -> Result<()> {
    let restore = if entered { execute!(io::stdout(), Show, DisableMouseCapture, LeaveAlternateScreen).err() } else { None };
    finish_tui_restore(restore, terminal::disable_raw_mode().err())
}
fn finish_tui_restore(restore: Option<io::Error>, disable: Option<io::Error>) -> Result<()> {
    match (restore, disable) { (None, None) => Ok(()), (Some(err), None) | (None, Some(err)) => Err(terminal_error(err)),
        (Some(a), Some(b)) => Err(AsciiAnimError::Terminal(format!("{a}; additionally failed to disable raw mode: {b}"))) }
}
fn terminal_error(err: io::Error) -> AsciiAnimError { AsciiAnimError::Terminal(err.to_string()) }
