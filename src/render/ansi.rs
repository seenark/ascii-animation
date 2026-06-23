use super::buffer::FrameBuffer;

pub fn render_to_ansi(frame: &FrameBuffer, color: bool) -> String {
    let mut out = String::new();
    for y in 0..frame.height() {
        if y > 0 {
            out.push('\n');
        }
        for x in 0..frame.width() {
            let cell = frame.get(x, y).expect("coordinates are inside frame");
            if color {
                if let Some(rgb) = cell.color {
                    out.push_str(&format!(
                        "\u{1b}[38;2;{};{};{}m{}\u{1b}[0m",
                        rgb.r, rgb.g, rgb.b, cell.ch
                    ));
                } else {
                    out.push(cell.ch);
                }
            } else {
                out.push(cell.ch);
            }
        }
    }
    out
}
