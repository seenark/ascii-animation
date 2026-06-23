pub mod ansi;
pub mod buffer;
pub mod layout;

pub use buffer::{Cell, FrameBuffer, Rgb};
pub use layout::{resolve_placement, Rect};
