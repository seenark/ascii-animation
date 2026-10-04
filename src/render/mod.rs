pub mod ansi;
pub mod buffer;
pub mod layout;

pub use buffer::{Cell, FrameBuffer, Rgb};
pub use layout::{resolve_placement, Rect};

use crate::scene::Layer;

#[derive(Debug, Clone, Copy)]
pub struct RenderContext {
    pub elapsed_seconds: f64,
    pub layer: Layer,
    pub z_index: i32,
    pub order: usize,
    pub x_offset: u16,
    pub y_offset: u16,
    pub width: u16,
    pub height: u16,
}

pub trait AnimationRenderer {
    fn render(&mut self, frame: &mut FrameBuffer, context: RenderContext);

    /// Apply already validated options without replacing retained simulation state.
    fn reconfigure(
        &mut self,
        _options: &std::collections::BTreeMap<String, crate::presets::OptionValue>,
    ) -> crate::Result<()> {
        Err(crate::AsciiAnimError::Terminal(
            "renderer does not support live reconfiguration".into(),
        ))
    }

    fn depends_on_dimensions(&self) -> bool {
        false
    }
}
