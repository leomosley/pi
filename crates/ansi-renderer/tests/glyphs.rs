use ansi_renderer::{Glyph, GlyphLibrary, GlyphSet, Terminal};

#[test]
fn ascii_preset_uses_only_ascii_characters() {
    let library = GlyphLibrary::preset(GlyphSet::Ascii);
    let frame = library.render_centered("12:34:56", 60, 40);
    assert!(frame.is_ascii() && frame.contains('#'));
}

#[test]
fn blocks_preset_preserves_solid_block_style() {
    let library = GlyphLibrary::preset(GlyphSet::Blocks);
    assert!(library.render_centered("12:34:56", 60, 40).contains('█'));
}

#[test]
fn segment_preset_uses_underscores_and_bars() {
    let library = GlyphLibrary::preset(GlyphSet::Segments);
    let frame = library.render_centered("12:34:56", 60, 40);
    assert!(frame.contains('_') && frame.contains('|') && !frame.contains('#'));
}

#[test]
fn custom_glyphs_are_centered_using_cell_dimensions() {
    let mut library = GlyphLibrary::preset(GlyphSet::Ascii);
    library.insert('X', Glyph::new(&["AB", "CD"]).unwrap());
    let mut terminal = Terminal::new(10, 4).unwrap();
    terminal.feed(library.render_centered("X", 10, 4).as_bytes());
    assert_eq!(
        (
            terminal.cell(4, 1).unwrap().character,
            terminal.cell(5, 2).unwrap().character
        ),
        ('A', 'D')
    );
}

#[test]
fn uppercase_and_lowercase_share_builtin_glyphs() {
    let library = GlyphLibrary::preset(GlyphSet::Ascii);
    assert_eq!(library.glyph('b').rows(), library.glyph('B').rows());
}

#[test]
fn custom_glyph_replaces_builtin() {
    let mut library = GlyphLibrary::preset(GlyphSet::Ascii);
    library.insert('0', Glyph::new(&["@@", "@@"]).unwrap());
    assert_eq!(library.glyph('0').rows(), &["@@", "@@"]);
}

#[test]
fn ragged_glyphs_are_rejected() {
    assert!(Glyph::new(&["###", "##"]).is_err());
}

#[test]
fn empty_glyphs_are_rejected() {
    assert!(Glyph::new(&[]).is_err());
}

#[test]
fn control_sequences_cannot_be_inserted_in_glyphs() {
    assert!(Glyph::new(&["\x1b[0m"]).is_err());
}

#[test]
fn small_viewport_falls_back_to_normal_text() {
    let library = GlyphLibrary::preset(GlyphSet::Ascii);
    assert!(
        library
            .render_centered("12:34:56", 30, 20)
            .contains("12:34:56")
    );
}

#[test]
fn long_fallback_text_is_clipped_without_scrolling() {
    let library = GlyphLibrary::preset(GlyphSet::Ascii);
    let mut terminal = Terminal::new(3, 2).unwrap();
    terminal.feed(library.render_centered("123456789", 3, 2).as_bytes());
    let row: String = (0..3)
        .map(|x| terminal.cell(x, 1).unwrap().character)
        .collect();
    assert_eq!(row, "123");
}

#[test]
fn every_builtin_glyph_is_rectangular() {
    for set in [GlyphSet::Ascii, GlyphSet::Blocks, GlyphSet::Segments] {
        let library = GlyphLibrary::preset(set);
        for character in "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ:? ".chars() {
            let glyph = library.glyph(character);
            assert!(
                glyph
                    .rows()
                    .iter()
                    .all(|row| row.chars().count() == glyph.width())
            );
        }
    }
}

#[test]
fn unknown_glyph_set_is_rejected() {
    assert!("unknown".parse::<GlyphSet>().is_err());
}
