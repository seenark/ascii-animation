use crate::scene::Placement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

pub fn resolve_placement(
    placement: &Placement,
    frame_width: u16,
    frame_height: u16,
    desired_width: u16,
    desired_height: u16,
) -> Rect {
    let width = desired_width.min(frame_width);
    let height = desired_height.min(frame_height);
    match placement {
        Placement::Center => Rect::new(
            (frame_width - width) / 2,
            (frame_height - height) / 2,
            width,
            height,
        ),
        Placement::Top => Rect::new((frame_width - width) / 2, 0, width, height),
        Placement::Bottom => Rect::new(
            (frame_width - width) / 2,
            frame_height - height,
            width,
            height,
        ),
        Placement::Left => Rect::new(0, (frame_height - height) / 2, width, height),
        Placement::Right => Rect::new(
            frame_width - width,
            (frame_height - height) / 2,
            width,
            height,
        ),
        Placement::Fill => Rect::new(0, 0, frame_width, frame_height),
        Placement::Custom {
            x,
            y,
            width,
            height,
        } => Rect::new(
            *x,
            *y,
            (*width).min(frame_width.saturating_sub(*x)),
            (*height).min(frame_height.saturating_sub(*y)),
        ),
    }
}
