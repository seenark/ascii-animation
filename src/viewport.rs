use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub const TUI_OPTIONS_PERCENT: u16 = 30;
pub const TUI_PREVIEW_PERCENT: u16 = 70;
pub const PREVIEW_BORDER_WIDTH: u16 = 2;
pub const PREVIEW_BORDER_HEIGHT: u16 = 2;

pub fn split_tui_layout(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(TUI_OPTIONS_PERCENT),
            Constraint::Percentage(TUI_PREVIEW_PERCENT),
        ])
        .split(area);
    (chunks[0], chunks[1])
}

pub fn animation_viewport_size_for_terminal(width: u16, height: u16) -> (u16, u16) {
    let (_, preview) = split_tui_layout(Rect::new(0, 0, width, height));
    (
        preview.width.saturating_sub(PREVIEW_BORDER_WIDTH).max(1),
        preview.height.saturating_sub(PREVIEW_BORDER_HEIGHT).max(1),
    )
}
