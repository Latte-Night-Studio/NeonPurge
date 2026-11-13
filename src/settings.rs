//! Persistent user settings: display mode, language and sound.
//!
//! The file lives next to the executable (like the high score), so a portable
//! build keeps its preferences with it. A missing or malformed file simply
//! falls back to the defaults; the game never refuses to start because of it.

use std::path::PathBuf;

use crate::lang::Lang;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Windowed,
    Borderless,
    Fullscreen,
}

impl DisplayMode {
    #[cfg(test)]
    pub const ALL: [DisplayMode; 3] = [
        DisplayMode::Windowed,
        DisplayMode::Borderless,
        DisplayMode::Fullscreen,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "windowed" | "window" | "віконний" => Some(DisplayMode::Windowed),
            "borderless" | "без рамки" => Some(DisplayMode::Borderless),
            "fullscreen" | "full screen" | "повноекранний" => Some(DisplayMode::Fullscreen),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            DisplayMode::Windowed => "windowed",
            DisplayMode::Borderless => "borderless",
            DisplayMode::Fullscreen => "fullscreen",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub display: DisplayMode,
    pub lang: Lang,
    pub sound: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            display: DisplayMode::Windowed,
            lang: Lang::Uk,
            sound: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = settings_path() {
            let _ = std::fs::write(path, self.serialize());
        }
    }

    /// Parse the `key=value` file format. Unknown keys are ignored so older
    /// builds keep working after new settings are added.
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "display" | "display_mode" | "mode" => {
                    if let Some(mode) = DisplayMode::parse(value) {
                        settings.display = mode;
                    }
                }
                "language" | "lang" => {
                    if let Some(lang) = Lang::parse(value) {
                        settings.lang = lang;
                    }
                }
                "sound" | "audio" => {
                    settings.sound = !matches!(
                        value.to_ascii_lowercase().as_str(),
                        "off" | "0" | "false" | "no" | "mute" | "muted"
                    );
                }
                _ => {}
            }
        }
        settings
    }

    pub fn serialize(&self) -> String {
        format!(
            "# NEON PURGE settings\ndisplay={}\nlanguage={}\nsound={}\n",
            self.display.code(),
            self.lang.code(),
            if self.sound { "on" } else { "off" }
        )
    }
}

fn settings_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.join("neon_purge_settings.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_display_mode_round_trips() {
        for mode in DisplayMode::ALL {
            assert_eq!(DisplayMode::parse(mode.code()), Some(mode));
        }
    }

    #[test]
    fn settings_round_trip() {
        let settings = Settings {
            display: DisplayMode::Fullscreen,
            lang: Lang::En,
            sound: false,
        };
        assert_eq!(Settings::parse(&settings.serialize()), settings);
    }

    #[test]
    fn malformed_lines_fall_back_to_defaults() {
        let parsed = Settings::parse("nonsense\n=broken\ndisplay=warp\ngarbage=1\n");
        assert_eq!(parsed, Settings::default());
    }

    #[test]
    fn partial_files_keep_other_values() {
        let parsed = Settings::parse("language=en\nsound=off\n");
        assert_eq!(parsed.display, DisplayMode::Windowed);
        assert_eq!(parsed.lang, Lang::En);
        assert!(!parsed.sound);
    }
}
