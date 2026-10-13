use ansi_renderer::{CELL_HEIGHT, CELL_WIDTH, GlyphLibrary, GlyphSet, Terminal};
use anyhow::{Context, Result, bail};
use screen_api::Screen;
use screen_clock::{ClockScreen, ClockSettings};
use screen_portfolio::{PortfolioScreen, PortfolioSettings, portfolio_glyphs};
use screen_r6::{R6Screen, R6Settings};
use screen_usage::{UsageScreen, UsageSettings};

use crate::{
    PlayerWindow,
    config::{DisplayProfile, Renderer, ScreenEntry},
};

pub struct AnsiSurface {
    terminal: Terminal,
    last_source: String,
    image: slint::Image,
    glyphs: GlyphLibrary,
}

impl AnsiSurface {
    fn new(display: &DisplayProfile, glyphs: GlyphSet) -> Result<Self> {
        Ok(Self {
            terminal: Terminal::new(
                (display.width_px / CELL_WIDTH) as usize,
                (display.height_px / CELL_HEIGHT) as usize,
            )
            .context("display is too large for the ANSI renderer")?,
            last_source: String::new(),
            image: slint::Image::default(),
            glyphs: GlyphLibrary::preset(glyphs),
        })
    }

    fn render(&mut self, source: &str) -> slint::Image {
        if self.last_source != source {
            self.terminal.reset();
            self.terminal.feed(source.as_bytes());
            let frame = self.terminal.rasterize();
            let mut buffer =
                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(frame.width, frame.height);
            buffer.make_mut_bytes().copy_from_slice(&frame.pixels);
            self.image = slint::Image::from_rgba8(buffer);
            self.last_source.clear();
            self.last_source.push_str(source);
        }
        self.image.clone()
    }
}

pub enum ScreenInstance {
    R6 {
        screen: R6Screen,
        ansi: AnsiSurface,
    },
    Portfolio {
        screen: PortfolioScreen,
        ansi: Option<AnsiSurface>,
    },
    Usage {
        screen: UsageScreen,
        ansi: Option<AnsiSurface>,
    },
    Clock {
        screen: ClockScreen,
        ansi: Option<AnsiSurface>,
    },
}

impl ScreenInstance {
    pub fn build(entry: &ScreenEntry, display: &DisplayProfile) -> Result<Self> {
        match entry.module.as_str() {
            "clock" => {
                let settings: ClockSettings = entry
                    .settings
                    .clone()
                    .try_into()
                    .context("invalid clock settings")?;
                let ansi = if entry.renderer == Renderer::Ansi {
                    Some(AnsiSurface::new(display, entry.glyphs)?)
                } else {
                    None
                };
                Ok(Self::Clock {
                    screen: ClockScreen::new(settings)?,
                    ansi,
                })
            }
            "usage" => {
                let settings: UsageSettings = entry
                    .settings
                    .clone()
                    .try_into()
                    .context("invalid usage settings")?;
                let ansi = if entry.renderer == Renderer::Ansi {
                    Some(AnsiSurface::new(display, entry.glyphs)?)
                } else {
                    None
                };
                Ok(Self::Usage {
                    screen: UsageScreen::new(settings)?,
                    ansi,
                })
            }
            "portfolio" => {
                let settings: PortfolioSettings = entry
                    .settings
                    .clone()
                    .try_into()
                    .context("invalid portfolio settings")?;
                let ansi = if entry.renderer == Renderer::Ansi {
                    let mut surface = AnsiSurface::new(display, entry.glyphs)?;
                    surface.glyphs = portfolio_glyphs(surface.glyphs)?;
                    Some(surface)
                } else {
                    None
                };
                Ok(Self::Portfolio {
                    screen: PortfolioScreen::new(settings)?,
                    ansi,
                })
            }
            "r6" => {
                let settings: R6Settings = entry
                    .settings
                    .clone()
                    .try_into()
                    .context("invalid r6 settings")?;
                Ok(Self::R6 {
                    screen: R6Screen::new(settings)?,
                    ansi: AnsiSurface::new(display, entry.glyphs)?,
                })
            }
            module => bail!(
                "unknown screen module '{module}'; registered modules: clock, usage, portfolio, r6"
            ),
        }
    }

    pub fn render(&mut self, window: &PlayerWindow) {
        match self {
            Self::R6 { screen, ansi } => {
                window.set_usage_mode(false);
                window.set_portfolio_mode(false);
                window.set_ansi_mode(true);
                let frame = screen.ansi_frame(ansi.terminal.columns(), ansi.terminal.rows());
                window.set_ansi_frame(ansi.render(&frame));
            }
            Self::Clock { screen, ansi } => {
                window.set_portfolio_mode(false);
                window.set_usage_mode(false);
                window.set_ansi_mode(ansi.is_some());
                if let Some(surface) = ansi {
                    let frame = screen.ansi_frame(
                        surface.terminal.columns(),
                        surface.terminal.rows(),
                        &surface.glyphs,
                    );
                    window.set_ansi_frame(surface.render(&frame));
                } else {
                    window.set_clock_time(screen.time().into());
                }
            }
            Self::Usage { screen, ansi } => {
                window.set_portfolio_mode(false);
                window.set_usage_mode(true);
                window.set_ansi_mode(ansi.is_some());
                if let Some(surface) = ansi {
                    let frame =
                        screen.ansi_frame(surface.terminal.columns(), surface.terminal.rows());
                    window.set_ansi_frame(surface.render(&frame));
                } else {
                    let usage = screen.usage();
                    window
                        .set_five_hour(usage.and_then(|u| u.five_hour).map_or(-1.0, |p| p as f32));
                    window.set_weekly(usage.and_then(|u| u.weekly).map_or(-1.0, |p| p as f32));
                    window.set_usage_message(screen.message().into());
                }
            }
            Self::Portfolio { screen, ansi } => {
                window.set_usage_mode(false);
                window.set_portfolio_mode(true);
                window.set_ansi_mode(ansi.is_some());
                if let Some(surface) = ansi {
                    let frame = screen.ansi_frame(
                        surface.terminal.columns(),
                        surface.terminal.rows(),
                        &surface.glyphs,
                    );
                    window.set_ansi_frame(surface.render(&frame));
                } else {
                    let amount = screen.display_amount();
                    window.set_portfolio_amount_length(amount.chars().count() as i32);
                    window.set_portfolio_amount(amount.into());
                    window.set_portfolio_refresh_mask(i32::from(screen.refresh_indicator().mask));
                    window.set_portfolio_message(screen.message().into());
                }
            }
        }
    }
}

impl Screen for ScreenInstance {
    fn activate(&mut self) {
        match self {
            Self::Clock { screen, .. } => screen.activate(),
            Self::Usage { screen, .. } => screen.activate(),
            Self::Portfolio { screen, .. } => screen.activate(),
            Self::R6 { screen, .. } => screen.activate(),
        }
    }

    fn tick(&mut self) {
        match self {
            Self::Clock { screen, .. } => screen.tick(),
            Self::Usage { screen, .. } => screen.tick(),
            Self::Portfolio { screen, .. } => screen.tick(),
            Self::R6 { screen, .. } => screen.tick(),
        }
    }

    fn deactivate(&mut self) {
        match self {
            Self::Clock { screen, .. } => screen.deactivate(),
            Self::Usage { screen, .. } => screen.deactivate(),
            Self::Portfolio { screen, .. } => screen.deactivate(),
            Self::R6 { screen, .. } => screen.deactivate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display() -> DisplayProfile {
        DisplayProfile {
            name: "Test".into(),
            width_px: 480,
            height_px: 320,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn unknown_module_is_rejected() {
        let entry = ScreenEntry {
            module: "missing".into(),
            duration_seconds: 10,
            renderer: Renderer::Native,
            glyphs: GlyphSet::Ascii,
            settings: toml::Table::new(),
        };
        assert!(ScreenInstance::build(&entry, &display()).is_err());
    }

    #[test]
    fn invalid_clock_format_is_rejected() {
        let entry: ScreenEntry =
            toml::from_str("module='clock'\nduration_seconds=10\n[settings]\nformat='invalid'")
                .unwrap();
        assert!(ScreenInstance::build(&entry, &display()).is_err());
    }
}
