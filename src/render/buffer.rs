use crate::scene::Layer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub color: Option<Rgb>,
    pub layer: Layer,
    pub z_index: i32,
    pub order: usize,
}

impl Cell {
    pub fn empty() -> Self {
        Self {
            ch: ' ',
            color: None,
            layer: Layer::Background,
            z_index: i32::MIN,
            order: 0,
        }
    }

    pub fn visible(ch: char, color: Option<Rgb>, layer: Layer, z_index: i32, order: usize) -> Self {
        Self { ch, color, layer, z_index, order }
    }

    fn is_transparent(self) -> bool {
        self.ch == ' '
    }

    fn should_replace(self, current: Self) -> bool {
        if self.is_transparent() {
            return false;
        }
        if current.is_transparent() {
            return true;
        }
        self.layer.priority() > current.layer.priority()
            || (self.layer.priority() == current.layer.priority() && self.z_index > current.z_index)
            || (self.layer.priority() == current.layer.priority()
                && self.z_index == current.z_index
                && self.order >= current.order)
    }
}

#[derive(Debug, Clone)]
pub struct FrameBuffer {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl FrameBuffer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::empty(); width as usize * height as usize],
        }
    }

    pub fn width(&self) -> u16 { self.width }
    pub fn height(&self) -> u16 { self.height }

    pub fn put_cell(&mut self, x: u16, y: u16, cell: Cell) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = self.index(x, y);
        if cell.should_replace(self.cells[idx]) {
            self.cells[idx] = cell;
        }
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells.get(self.index(x, y))
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for y in 0..self.height {
            if y > 0 {
                out.push('\n');
            }
            for x in 0..self.width {
                out.push(self.get(x, y).map(|cell| cell.ch).unwrap_or(' '));
            }
        }
        out
    }

    fn index(&self, x: u16, y: u16) -> usize {
        y as usize * self.width as usize + x as usize
    }
}
