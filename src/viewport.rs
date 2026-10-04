use ratatui::layout::Rect;

pub const PREVIEW_BORDER_WIDTH: u16 = 2;
pub const PREVIEW_BORDER_HEIGHT: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorLayout { Wide, Medium, Small, Tiny }

pub fn editor_layout(width: u16, height: u16) -> EditorLayout {
    if width < 36 || height < 10 { EditorLayout::Tiny }
    else if width >= 110 && height >= 24 { EditorLayout::Wide }
    else if width >= 80 && height >= 24 { EditorLayout::Medium }
    else { EditorLayout::Small }
}

// All consumers use the normal Preview view, even while a small editor hides it.
pub fn split_tui_layout(area: Rect) -> (Rect, Rect) {
    let body = Rect::new(area.x, area.y.saturating_add(1), area.width, area.height.saturating_sub(4));
    match editor_layout(area.width, area.height) {
        EditorLayout::Wide => {
            let sidebar = (area.width * 30 / 100).clamp(32, 42);
            (Rect::new(body.x, body.y, sidebar, body.height),
             Rect::new(body.x + sidebar, body.y, body.width.saturating_sub(sidebar), body.height))
        }
        EditorLayout::Medium => {
            let preview_height = (body.height / 2).max(8).min(body.height);
            (Rect::new(body.x, body.y + preview_height, body.width, body.height.saturating_sub(preview_height)),
             Rect::new(body.x, body.y, body.width, preview_height))
        }
        EditorLayout::Small | EditorLayout::Tiny => (body, body),
    }
}

pub fn animation_viewport_size_for_terminal(width: u16, height: u16) -> (u16, u16) {
    let (_, preview) = split_tui_layout(Rect::new(0, 0, width, height));
    (preview.width.saturating_sub(PREVIEW_BORDER_WIDTH).max(1),
     preview.height.saturating_sub(PREVIEW_BORDER_HEIGHT).max(1))
}
