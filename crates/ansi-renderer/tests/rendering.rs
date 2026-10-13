use ansi_renderer::{Color, RenderError, Style, Terminal};

fn screen(input: &str) -> Terminal {
    let mut screen = Terminal::new(10, 4).unwrap();
    screen.feed(input.as_bytes());
    screen
}

#[test]
fn cursor_coordinates_are_one_based() {
    let screen = screen("\x1b[2;3HX");
    assert_eq!(screen.cell(2, 1).unwrap().character, 'X');
}

#[test]
fn movement_defaults_to_one_cell() {
    let screen = screen("\x1b[2;3H\x1b[AX");
    assert_eq!(screen.cell(2, 0).unwrap().character, 'X');
}

#[test]
fn sixteen_colour_sgr_sets_foreground() {
    let screen = screen("\x1b[91mX");
    assert_eq!(
        screen.cell(0, 0).unwrap().style.foreground,
        Color(255, 0, 0)
    );
}

#[test]
fn indexed_colour_sgr_sets_foreground() {
    let screen = screen("\x1b[38;5;46mX");
    assert_eq!(
        screen.cell(0, 0).unwrap().style.foreground,
        Color(0, 255, 0)
    );
}

#[test]
fn rgb_sgr_sets_background() {
    let screen = screen("\x1b[48;2;12;34;56mX");
    assert_eq!(
        screen.cell(0, 0).unwrap().style.background,
        Color(12, 34, 56)
    );
}

#[test]
fn reset_restores_default_style() {
    let screen = screen("\x1b[1;7;31;44mA\x1b[0mB");
    assert_eq!(screen.cell(1, 0).unwrap().style, Style::default());
}

#[test]
fn erase_line_preserves_other_lines() {
    let screen = screen("ABC\r\nDEF\x1b[2K");
    let chars: String = (0..3)
        .map(|x| screen.cell(x, 0).unwrap().character)
        .collect();
    assert_eq!(chars, "ABC");
}

#[test]
fn erase_screen_clears_all_cells() {
    let screen = screen("ABC\r\nDEF\x1b[2J");
    assert!((0..4).all(|y| (0..10).all(|x| screen.cell(x, y).unwrap().character == ' ')));
}

#[test]
fn split_escape_sequences_are_parsed_incrementally() {
    let mut screen = Terminal::new(10, 4).unwrap();
    screen.feed(b"\x1b[38;2;12;");
    screen.feed(b"34;56mX");
    assert_eq!(
        screen.cell(0, 0).unwrap().style.foreground,
        Color(12, 34, 56)
    );
}

#[test]
fn split_utf8_block_is_parsed_incrementally() {
    let mut screen = Terminal::new(10, 4).unwrap();
    let bytes = "█".as_bytes();
    screen.feed(&bytes[..1]);
    screen.feed(&bytes[1..]);
    assert_eq!(screen.cell(0, 0).unwrap().character, '█');
}

#[test]
fn reset_discards_incomplete_escape_sequence() {
    let mut screen = screen("\x1b[38;2;");
    screen.reset();
    screen.feed(b"X");
    assert_eq!(screen.cell(0, 0).unwrap().character, 'X');
}

#[test]
fn scrolling_keeps_last_line() {
    let mut screen = Terminal::new(3, 2).unwrap();
    screen.feed(b"one\r\ntwo\r\nnew");
    let row: String = (0..3)
        .map(|x| screen.cell(x, 0).unwrap().character)
        .collect();
    assert_eq!(row, "two");
}

#[test]
fn sgr_preserves_pending_wrap() {
    let mut screen = Terminal::new(2, 2).unwrap();
    screen.feed(b"AB\x1b[31mC");
    assert_eq!(screen.cell(0, 1).unwrap().character, 'C');
}

#[test]
fn save_and_restore_cursor() {
    let screen = screen("\x1b[2;3H\x1b[s\x1b[4;10H\x1b[uX");
    assert_eq!(screen.cell(2, 1).unwrap().character, 'X');
}

#[test]
fn huge_cursor_coordinates_are_clamped() {
    let screen = screen("\x1b[65535;65535HX");
    assert_eq!(screen.cell(9, 3).unwrap().character, 'X');
}

#[test]
fn zero_dimensions_are_rejected() {
    assert!(matches!(
        Terminal::new(0, 1),
        Err(RenderError::InvalidDimensions)
    ));
}

#[test]
fn allocation_overflow_is_rejected() {
    assert!(matches!(
        Terminal::new(usize::MAX, 2),
        Err(RenderError::InvalidDimensions)
    ));
}

#[test]
fn oversized_grid_is_rejected() {
    assert!(matches!(
        Terminal::new(1024, 1024),
        Err(RenderError::InvalidDimensions)
    ));
}

#[test]
fn raster_dimensions_follow_cell_dimensions() {
    let screen = Terminal::new(60, 40).unwrap();
    let frame = screen.rasterize();
    assert_eq!(
        (frame.width, frame.height, frame.pixels.len()),
        (480, 320, 480 * 320 * 4)
    );
}

#[test]
fn full_block_has_only_foreground_pixels() {
    let mut screen = Terminal::new(1, 1).unwrap();
    screen.feed("\x1b[38;2;12;34;56m█".as_bytes());
    assert!(
        screen
            .rasterize()
            .pixels
            .chunks_exact(4)
            .all(|pixel| pixel == [12, 34, 56, 255])
    );
}

#[test]
fn inverse_swaps_foreground_and_background() {
    let mut screen = Terminal::new(1, 1).unwrap();
    screen.feed(b"\x1b[7m ");
    assert!(
        screen
            .rasterize()
            .pixels
            .chunks_exact(4)
            .all(|pixel| pixel == [255; 4])
    );
}

#[test]
fn glyph_edges_have_no_intermediate_colours() {
    let frame = screen("Clock 12:34").rasterize();
    assert!(
        frame
            .pixels
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 255] || pixel == [255; 4])
    );
}

#[test]
fn palette_extremes_are_defined() {
    assert_eq!(
        (Color::indexed(16), Color::indexed(231), Color::indexed(255)),
        (Color::BLACK, Color::WHITE, Color(238, 238, 238))
    );
}

#[test]
fn unsupported_osc_does_not_print_control_payload() {
    let screen = screen("\x1b]52;c;not-a-clipboard-action\x07X");
    assert_eq!(screen.cell(0, 0).unwrap().character, 'X');
}
