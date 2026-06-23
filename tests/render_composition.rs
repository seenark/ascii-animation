use ascii_animation::render::buffer::{Cell, FrameBuffer, Rgb};
use ascii_animation::scene::Layer;

fn cell(ch: char, layer: Layer, z_index: i32, order: usize) -> Cell {
    Cell::visible(ch, Some(Rgb::new(1, 2, 3)), layer, z_index, order)
}

#[test]
fn foreground_overrides_normal_and_background() {
    let mut frame = FrameBuffer::new(1, 1);

    frame.put_cell(0, 0, cell('b', Layer::Background, 100, 0));
    frame.put_cell(0, 0, cell('n', Layer::Normal, 0, 1));
    frame.put_cell(0, 0, cell('f', Layer::Foreground, -100, 2));

    assert_eq!(frame.get(0, 0).unwrap().ch, 'f');
}

#[test]
fn higher_z_index_wins_inside_same_layer() {
    let mut frame = FrameBuffer::new(1, 1);

    frame.put_cell(0, 0, cell('a', Layer::Normal, 1, 0));
    frame.put_cell(0, 0, cell('b', Layer::Normal, 2, 1));

    assert_eq!(frame.get(0, 0).unwrap().ch, 'b');
}

#[test]
fn later_instance_order_breaks_same_layer_and_z_ties() {
    let mut frame = FrameBuffer::new(1, 1);

    frame.put_cell(0, 0, cell('a', Layer::Normal, 1, 0));
    frame.put_cell(0, 0, cell('b', Layer::Normal, 1, 1));

    assert_eq!(frame.get(0, 0).unwrap().ch, 'b');
}

#[test]
fn equal_instance_order_does_not_break_ties() {
    let mut frame = FrameBuffer::new(1, 1);

    frame.put_cell(0, 0, cell('a', Layer::Normal, 1, 0));
    frame.put_cell(0, 0, cell('b', Layer::Normal, 1, 0));

    assert_eq!(frame.get(0, 0).unwrap().ch, 'a');
}

#[test]
fn spaces_are_transparent() {
    let mut frame = FrameBuffer::new(1, 1);

    frame.put_cell(0, 0, cell('x', Layer::Normal, 0, 0));
    frame.put_cell(0, 0, cell(' ', Layer::Foreground, 100, 1));

    assert_eq!(frame.get(0, 0).unwrap().ch, 'x');
}

#[test]
fn plain_text_preserves_frame_dimensions() {
    let mut frame = FrameBuffer::new(3, 2);
    frame.put_cell(1, 0, cell('x', Layer::Normal, 0, 0));
    frame.put_cell(2, 1, cell('y', Layer::Normal, 0, 0));

    assert_eq!(frame.to_plain_text(), " x \n  y");
}
