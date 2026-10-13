//! Explicit opt-in tests that launch the actual native window and inspect its pixels.

use std::{fs, io::BufReader, path::PathBuf, process::Command};

fn capture(name: &str, arguments: &[&str]) -> (png::OutputInfo, Vec<u8>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::var_os("PI_DISPLAY_TEST_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("pi-display-smoke"));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("{name}-{}.png", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_player"))
        .current_dir(root)
        .args(["--preview", "--capture"])
        .arg(&path)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut reader = png::Decoder::new(BufReader::new(fs::File::open(path).unwrap()))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    pixels.truncate(info.buffer_size());
    (info, pixels)
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn portrait_preview_preserves_target_dimensions() {
    let (info, _) = capture(
        "portrait",
        &["--display", "profiles/displays/example-480x800.toml"],
    );
    assert_eq!((info.width, info.height), (480, 848));
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn usage_screen_renders_two_ascii_percentage_bars() {
    let (info, pixels) = capture(
        "usage",
        &[
            "--display",
            "profiles/displays/dfrobot-fit0820-480x320.toml",
            "--playlist",
            "config/usage-demo.toml",
        ],
    );
    assert_eq!((info.width, info.height), (480, 368));
    let mut terminal = ansi_renderer::Terminal::new(60, 40).unwrap();
    let screen = screen_usage::UsageScreen::new(screen_usage::UsageSettings {
        demo: true,
        ..Default::default()
    })
    .unwrap();
    terminal.feed(screen.ansi_frame(60, 40).as_bytes());
    let expected = terminal.rasterize();
    let mismatch = pixels[..480 * 320 * 4]
        .chunks_exact(4)
        .zip(expected.pixels.chunks_exact(4))
        .enumerate()
        .find(|(_, (actual, expected))| actual != expected);
    assert!(mismatch.is_none(), "first mismatched pixel: {mismatch:?}");
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn usage_screen_also_supports_native_bars() {
    let (_, pixels) = capture(
        "usage-native",
        &[
            "--display",
            "profiles/displays/dfrobot-fit0820-480x320.toml",
            "--playlist",
            "config/usage-demo.toml",
            "--renderer",
            "native",
        ],
    );
    assert!(
        pixels[..480 * 320 * 4]
            .chunks_exact(4)
            .filter(|p| p[..3] == [255; 3])
            .count()
            > 3000
    );
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn portfolio_ascii_frame_matches_native_window_pixels() {
    let (info, pixels) = capture(
        "portfolio",
        &[
            "--display",
            "profiles/displays/dfrobot-fit0820-480x320.toml",
            "--playlist",
            "config/portfolio-demo.toml",
        ],
    );
    assert_eq!((info.width, info.height), (480, 368));
    let screen = screen_portfolio::PortfolioScreen::new(screen_portfolio::PortfolioSettings {
        demo: true,
        ..Default::default()
    })
    .unwrap();
    let glyphs = screen_portfolio::portfolio_glyphs(ansi_renderer::GlyphLibrary::preset(
        ansi_renderer::GlyphSet::Ascii,
    ))
    .unwrap();
    let mut terminal = ansi_renderer::Terminal::new(60, 40).unwrap();
    terminal.feed(screen.ansi_frame(60, 40, &glyphs).as_bytes());
    assert!(pixels[..480 * 320 * 4] == terminal.rasterize().pixels);
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn portfolio_native_amount_is_visible() {
    let (_, pixels) = capture(
        "portfolio-native",
        &[
            "--display",
            "profiles/displays/dfrobot-fit0820-480x320.toml",
            "--playlist",
            "config/portfolio-demo.toml",
            "--renderer",
            "native",
        ],
    );
    assert!(
        pixels[..480 * 320 * 4]
            .chunks_exact(4)
            .filter(|p| p[..3].iter().all(|&v| v > 200))
            .count()
            > 500
    );
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn r6_emblem_and_stats_match_native_pixels() {
    let (info, pixels) = capture(
        "r6",
        &[
            "--display",
            "profiles/displays/dfrobot-fit0820-480x320.toml",
            "--playlist",
            "config/r6-demo.toml",
        ],
    );
    assert_eq!((info.width, info.height), (480, 368));
    let screen = screen_r6::R6Screen::new(screen_r6::R6Settings {
        demo: true,
        ..Default::default()
    })
    .unwrap();
    let mut terminal = ansi_renderer::Terminal::new(60, 40).unwrap();
    terminal.feed(screen.ansi_frame(60, 40).as_bytes());
    assert!(pixels[..480 * 320 * 4] == terminal.rasterize().pixels);
    assert!(
        pixels[..480 * 320 * 4]
            .chunks_exact(4)
            .any(|p| p[2] > p[0] && p[0] > 80)
    );
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn scale_two_preserves_physical_target_dimensions() {
    let (info, _) = capture(
        "scale2",
        &["--display", "apps/player/tests/fixtures/scale2.toml"],
    );
    assert_eq!((info.width, info.height), (800, 576));
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn clock_digits_remain_visible_after_automatic_format_transition() {
    let (info, pixels) = capture(
        "rotation",
        &[
            "--playlist",
            "apps/player/tests/fixtures/rotation.toml",
            "--renderer",
            "native",
        ],
    );
    assert_eq!(info.color_type, png::ColorType::Rgba);
    // The digits occupy the left half of the centered clock. A Slint 1.18.1
    // Parley regression rendered only the AM/PM suffix after this transition.
    let white_pixels = (160..320)
        .flat_map(|y| (160..470).map(move |x| (y * info.width as usize + x) * 4))
        .filter(|&offset| {
            pixels[offset..offset + 3]
                .iter()
                .all(|&channel| channel > 200)
        })
        .count();
    assert!(
        white_pixels > 1000,
        "clock digits missing; found {white_pixels} white pixels"
    );
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn ansi_clock_is_rendered_as_exact_black_and_white_blocks() {
    let (info, pixels) = capture(
        "ansi",
        &[
            "--display",
            "profiles/displays/dfrobot-fit0820-480x320.toml",
            "--renderer",
            "ansi",
            "--glyphs",
            "blocks",
        ],
    );
    assert_eq!((info.width, info.height), (480, 368));
    let content = &pixels[..480 * 320 * 4];
    assert!(
        content
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 255] || pixel == [255; 4])
    );
    assert!(
        content
            .chunks_exact(4)
            .filter(|pixel| *pixel == [255; 4])
            .count()
            > 1000
    );
    // Every clock block occupies one exact 8×8 physical-pixel cell.
    for cell_y in 0..40 {
        for cell_x in 0..60 {
            let first = ((cell_y * 8) * 480 + cell_x * 8) * 4;
            for y in 0..8 {
                for x in 0..8 {
                    let offset = ((cell_y * 8 + y) * 480 + cell_x * 8 + x) * 4;
                    assert_eq!(&pixels[offset..offset + 4], &pixels[first..first + 4]);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires a Wayland or X11 desktop; run bun run smoke"]
fn ascii_clock_uses_hash_characters_rather_than_solid_blocks() {
    let (info, pixels) = capture(
        "ascii",
        &[
            "--display",
            "profiles/displays/dfrobot-fit0820-480x320.toml",
            "--renderer",
            "ansi",
            "--glyphs",
            "ascii",
        ],
    );
    assert_eq!((info.width, info.height), (480, 368));
    let mut character = ansi_renderer::Terminal::new(1, 1).unwrap();
    character.feed(b"#");
    let hash = character.rasterize();
    let mut hash_cells = 0;
    for cell_y in 0..40 {
        for cell_x in 0..60 {
            let mut actual = Vec::with_capacity(8 * 8 * 4);
            for y in 0..8 {
                let offset = ((cell_y * 8 + y) * 480 + cell_x * 8) * 4;
                actual.extend_from_slice(&pixels[offset..offset + 8 * 4]);
            }
            if actual.chunks_exact(4).any(|pixel| pixel[..3] != [0; 3]) {
                assert_eq!(
                    actual, hash.pixels,
                    "nonblank cell must contain the actual '#' glyph"
                );
                hash_cells += 1;
            }
        }
    }
    assert!(
        hash_cells > 20,
        "clock art should contain readable ASCII digits"
    );
}
