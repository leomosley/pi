//! Deterministic minute-rollover preview using the same glyph animation as the player.

use std::{borrow::Cow, fs::File, path::PathBuf};

use ansi_renderer::{GlyphLibrary, GlyphSet, Terminal};
use anyhow::Result;
use gif::{Encoder, Frame, Repeat};

fn main() -> Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/clock-roll.gif"));
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = File::create(&path)?;
    let mut encoder = Encoder::new(&mut file, 480, 320, &[0, 0, 0, 255, 255, 255])?;
    encoder.set_repeat(Repeat::Infinite)?;
    let glyphs = GlyphLibrary::preset(GlyphSet::Ascii);
    let mut terminal = Terminal::new(60, 40)?;
    for step in 0..=12 {
        let source = glyphs.render_rolling("12:59:59", "13:00:00", step as f32 / 12.0, 60, 40);
        terminal.reset();
        terminal.feed(source.as_bytes());
        let pixels = terminal.rasterize();
        let indexed = pixels
            .pixels
            .chunks_exact(4)
            .map(|pixel| u8::from(pixel[0] > 0))
            .collect();
        let frame = Frame {
            width: 480,
            height: 320,
            delay: if step == 0 {
                80
            } else if step == 12 {
                100
            } else {
                3
            },
            buffer: Cow::Owned(indexed),
            ..Frame::default()
        };
        encoder.write_frame(&frame)?;
    }
    println!("Saved rolling clock preview to {}", path.display());
    Ok(())
}
