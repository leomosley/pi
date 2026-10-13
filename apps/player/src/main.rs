mod config;
mod playlist;
mod preview;
mod registry;

use std::{cell::RefCell, path::PathBuf, process::ExitCode, rc::Rc, time::Instant};

use ansi_renderer::GlyphSet;
use anyhow::Result;
use clap::Parser;
use screen_api::Screen;
use slint::{ComponentHandle, Timer, TimerMode};

use config::{DisplayProfile, PlaylistConfig, Renderer, read_config};
use playlist::Playlist;
use registry::ScreenInstance;

slint::include_modules!();

#[derive(Parser)]
#[command(version, about = "Native screen player for Linux and Raspberry Pi")]
struct Args {
    /// Show preview controls below the target display. This is the default mode.
    #[arg(long, conflicts_with = "fullscreen")]
    preview: bool,
    /// Hide preview controls and fill the connected display.
    #[arg(long)]
    fullscreen: bool,
    #[arg(long, default_value = "profiles/displays/example-800x480.toml")]
    display: PathBuf,
    #[arg(long, default_value = "config/playlist.toml")]
    playlist: PathBuf,
    /// Override the renderer for all playlist entries.
    #[arg(long, value_enum)]
    renderer: Option<Renderer>,
    /// ANSI character-art library: ascii, blocks, or segments. Overrides playlist choices.
    #[arg(long)]
    glyphs: Option<GlyphSet>,
    /// Validate configuration and registered screens without opening a window.
    #[arg(long)]
    validate: bool,
    /// Save an app-only PNG after rendering, then exit. Requires a desktop session.
    #[arg(long, conflicts_with = "validate")]
    capture: Option<PathBuf>,
}

struct PlayerState {
    playlist: Playlist,
    screens: Vec<ScreenInstance>,
    labels: Vec<String>,
    last_tick: Instant,
}

impl PlayerState {
    fn new(config: &PlaylistConfig, display: &DisplayProfile) -> Result<Self> {
        let playlist = Playlist::new(config)?;
        let mut screens = config
            .screens
            .iter()
            .map(|entry| ScreenInstance::build(entry, display))
            .collect::<Result<Vec<_>>>()?;
        screens[0].activate();
        Ok(Self {
            playlist,
            screens,
            labels: config.screens.iter().map(|s| s.module.clone()).collect(),
            last_tick: Instant::now(),
        })
    }

    fn navigate(&mut self, forward: bool) {
        self.screens[self.playlist.index()].deactivate();
        if forward {
            self.playlist.next();
        } else {
            self.playlist.previous();
        }
        self.screens[self.playlist.index()].activate();
        self.last_tick = Instant::now();
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let previous = self.playlist.index();
        if self.playlist.tick(now.duration_since(self.last_tick)) {
            self.screens[previous].deactivate();
            self.screens[self.playlist.index()].activate();
        }
        self.last_tick = now;
        self.screens[self.playlist.index()].tick();
    }

    fn render(&mut self, window: &PlayerWindow) {
        let index = self.playlist.index();
        self.screens[index].render(window);
        window.set_paused(self.playlist.paused());
        let remaining = self.playlist.remaining().as_secs_f64().ceil() as u64;
        window.set_status(
            format!(
                "{} / {} | {} | {remaining}s",
                index + 1,
                self.screens.len(),
                self.labels[index]
            )
            .into(),
        );
    }
}

impl Drop for PlayerState {
    fn drop(&mut self) {
        self.screens[self.playlist.index()].deactivate();
    }
}

fn main() -> Result<ExitCode> {
    let args = Args::parse();
    let display: DisplayProfile = read_config(&args.display)?;
    display.validate()?;
    let mut config: PlaylistConfig = read_config(&args.playlist)?;
    if let Some(renderer) = args.renderer {
        for entry in &mut config.screens {
            entry.renderer = renderer;
        }
    }
    if let Some(glyphs) = args.glyphs {
        for entry in &mut config.screens {
            entry.glyphs = glyphs;
        }
    }
    let state = Rc::new(RefCell::new(PlayerState::new(&config, &display)?));

    if args.validate {
        println!(
            "Valid configuration: {} screens, {}",
            config.screens.len(),
            display.name
        );
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(code) = preview::launch_with_scale(&display)? {
        return Ok(code);
    }

    let window = PlayerWindow::new()?;
    window.set_preview(args.preview || !args.fullscreen);
    window.set_viewport_width(display.width_px as f32 / display.scale_factor);
    window.set_viewport_height(display.height_px as f32 / display.scale_factor);
    window.window().set_fullscreen(args.fullscreen);
    state.borrow_mut().render(&window);

    for forward in [false, true] {
        let state = Rc::clone(&state);
        let weak = window.as_weak();
        let navigate = move || {
            let Some(window) = weak.upgrade() else { return };
            let mut state = state.borrow_mut();
            state.navigate(forward);
            state.render(&window);
        };
        if forward {
            window.on_next(navigate);
        } else {
            window.on_previous(navigate);
        }
    }
    {
        let state = Rc::clone(&state);
        let weak = window.as_weak();
        window.on_toggle_pause(move || {
            let Some(window) = weak.upgrade() else { return };
            let mut state = state.borrow_mut();
            state.playlist.toggle_pause();
            state.last_tick = Instant::now();
            state.render(&window);
        });
    }

    let timer = Timer::default();
    let weak = window.as_weak();
    timer.start(
        TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            let Some(window) = weak.upgrade() else { return };
            let mut state = state.borrow_mut();
            state.tick();
            state.render(&window);
        },
    );

    println!(
        "Running {} at {}x{} physical pixels, scale {}. Preview controls are outside the display area.",
        display.name, display.width_px, display.height_px, display.scale_factor
    );
    let capture_error = Rc::new(RefCell::new(None));
    if let Some(path) = args.capture {
        let weak = window.as_weak();
        let capture_error = Rc::clone(&capture_error);
        Timer::single_shot(std::time::Duration::from_millis(1500), move || {
            if let Some(window) = weak.upgrade()
                && let Err(error) = preview::capture(&window, &path)
            {
                *capture_error.borrow_mut() = Some(error);
            }
            if let Err(error) = slint::quit_event_loop() {
                *capture_error.borrow_mut() = Some(error.into());
            }
        });
    }
    window.run()?;
    if let Some(error) = capture_error.borrow_mut().take() {
        return Err(error);
    }
    Ok(ExitCode::SUCCESS)
}
