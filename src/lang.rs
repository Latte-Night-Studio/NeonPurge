//! Localised interface strings.
//!
//! The game ships with English and Ukrainian. Every string is uppercase
//! because the built-in 5x7 bitmap font only has uppercase glyphs (Latin and
//! Cyrillic). Add new strings to both `EN` and `UK` and they will be picked up
//! by the glyph-coverage test in this module.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Uk,
    En,
}

impl Lang {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "uk" | "ua" | "ukrainian" | "ukrainska" => Some(Lang::Uk),
            "en" | "eng" | "english" => Some(Lang::En),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Uk => "uk",
            Lang::En => "en",
        }
    }
}

pub struct Text {
    // Menus.
    pub play: &'static str,
    pub settings: &'static str,
    pub quit: &'static str,
    pub back: &'static str,
    pub resume: &'static str,
    pub restart: &'static str,
    pub main_menu: &'static str,
    pub retry: &'static str,

    // Title screen.
    pub subtitle: &'static str,
    pub controls: [(&'static str, &'static str); 8],
    pub esc_hint: &'static str,
    pub start_hint: &'static str,

    // Settings.
    pub display_mode: &'static str,
    pub windowed: &'static str,
    pub borderless: &'static str,
    pub fullscreen: &'static str,
    pub resolution: &'static str,
    pub resolution_note: &'static str,
    pub language: &'static str,
    pub lang_uk: &'static str,
    pub lang_en: &'static str,
    pub sound: &'static str,
    pub on: &'static str,
    pub off: &'static str,
    pub esc_back: &'static str,

    // HUD and gameplay labels.
    pub wave: &'static str,
    pub score: &'static str,
    pub best: &'static str,
    pub kills: &'static str,
    pub combo: &'static str,
    pub hp: &'static str,
    pub dash: &'static str,
    pub reloading: &'static str,
    pub sound_off: &'static str,
    pub hint: &'static str,
    pub rifle: &'static str,
    pub shotgun: &'static str,

    // Banners and screens.
    pub paused: &'static str,
    pub died: &'static str,
    pub new_record: &'static str,
    pub elite: &'static str,
    pub cleared: &'static str,
    pub bonus: &'static str,
    pub ammo: &'static str,
    pub heal: &'static str,
    pub esc_resume: &'static str,
    pub esc_menu: &'static str,
}

pub const EN: Text = Text {
    play: "PLAY",
    settings: "SETTINGS",
    quit: "QUIT",
    back: "BACK",
    resume: "RESUME",
    restart: "RESTART",
    main_menu: "MAIN MENU",
    retry: "RETRY",

    subtitle: "ARENA SHOOTER",
    controls: [
        ("WASD / ARROWS", "MOVE"),
        ("MOUSE", "AIM"),
        ("LEFT MOUSE", "FIRE"),
        ("SHIFT / SPACE", "DASH"),
        ("R", "RELOAD"),
        ("Q", "SWAP WEAPON"),
        ("P / ESC", "PAUSE"),
        ("M", "SOUND"),
    ],
    esc_hint: "ESC QUITS",
    start_hint: "CLICK A BUTTON OR PRESS ENTER",

    display_mode: "DISPLAY MODE",
    windowed: "WINDOWED",
    borderless: "BORDERLESS",
    fullscreen: "FULLSCREEN",
    resolution: "RESOLUTION",
    resolution_note: "FULLSCREEN ALWAYS USES THE MONITOR RESOLUTION",
    language: "LANGUAGE",
    lang_uk: "UKRAINIAN",
    lang_en: "ENGLISH",
    sound: "SOUND",
    on: "ON",
    off: "OFF",
    esc_back: "ESC - BACK",

    wave: "WAVE",
    score: "SCORE",
    best: "BEST",
    kills: "KILLS",
    combo: "COMBO",
    hp: "HP",
    dash: "DASH",
    reloading: "RELOADING",
    sound_off: "SOUND OFF",
    hint: "WASD MOVE   LMB FIRE   SHIFT DASH   R RELOAD   Q WEAPON   P PAUSE   M SOUND",
    rifle: "RIFLE",
    shotgun: "SHOTGUN",

    paused: "PAUSED",
    died: "YOU DIED",
    new_record: "NEW HIGH SCORE",
    elite: "ELITE",
    cleared: "CLEARED",
    bonus: "BONUS",
    ammo: "AMMO",
    heal: "+30 HP",
    esc_resume: "ESC / P - RESUME",
    esc_menu: "ESC - MAIN MENU",
};

pub const UK: Text = Text {
    play: "ГРАТИ",
    settings: "НАЛАШТУВАННЯ",
    quit: "ВИХІД",
    back: "НАЗАД",
    resume: "ПРОДОВЖИТИ",
    restart: "СПОЧАТКУ",
    main_menu: "ГОЛОВНЕ МЕНЮ",
    retry: "ЩЕ РАЗ",

    subtitle: "АРЕНА-ШУТЕР",
    controls: [
        ("WASD / СТРІЛКИ", "РУХ"),
        ("МИША", "ПРИЦІЛ"),
        ("ЛКМ", "ВОГОНЬ"),
        ("SHIFT / SPACE", "РИВОК"),
        ("R", "ПЕРЕЗАРЯДКА"),
        ("Q", "ЗБРОЯ"),
        ("P / ESC", "ПАУЗА"),
        ("M", "ЗВУК"),
    ],
    esc_hint: "ESC - ВИХІД",
    start_hint: "ОБЕРІТЬ ПУНКТ АБО ENTER",

    display_mode: "РЕЖИМ ВІКНА",
    windowed: "ВІКОННИЙ",
    borderless: "БЕЗ РАМКИ",
    fullscreen: "ПОВНОЕКРАННИЙ",
    resolution: "РОЗДІЛЬНА ЗДАТНІСТЬ",
    resolution_note: "ПОВНОЕКРАННИЙ - РОЗДІЛЬНІСТЬ МОНІТОРА",
    language: "МОВА",
    lang_uk: "УКРАЇНСЬКА",
    lang_en: "АНГЛІЙСЬКА",
    sound: "ЗВУК",
    on: "УВІМК",
    off: "ВИМК",
    esc_back: "ESC - НАЗАД",

    wave: "ХВИЛЯ",
    score: "ОЧКИ",
    best: "РЕКОРД",
    kills: "ВБИТО",
    combo: "КОМБО",
    hp: "ЖИТТЯ",
    dash: "РИВОК",
    reloading: "ПЕРЕЗАРЯДКА",
    sound_off: "ЗВУК ВИМК",
    hint: "WASD РУХ   ЛКМ ВОГОНЬ   SHIFT РИВОК   R ПЕРЕЗАРЯДКА   Q ЗБРОЯ   P ПАУЗА   M ЗВУК",
    rifle: "ГВИНТІВКА",
    shotgun: "ДРОБОВИК",

    paused: "ПАУЗА",
    died: "ВАС ЗНИЩЕНО",
    new_record: "НОВИЙ РЕКОРД",
    elite: "ЕЛІТА",
    cleared: "ЗАЧИЩЕНО",
    bonus: "БОНУС",
    ammo: "НАБОЇ",
    heal: "+30 ЖИТТЯ",
    esc_resume: "ESC / P - ПРОДОВЖИТИ",
    esc_menu: "ESC - ГОЛОВНЕ МЕНЮ",
};

pub fn text(lang: Lang) -> &'static Text {
    match lang {
        Lang::Uk => &UK,
        Lang::En => &EN,
    }
}

impl Text {
    /// Every string in this catalogue, used by the font coverage test.
    #[cfg(test)]
    pub fn all(&self) -> Vec<&'static str> {
        let mut all = vec![
            self.play,
            self.settings,
            self.quit,
            self.back,
            self.resume,
            self.restart,
            self.main_menu,
            self.retry,
            self.subtitle,
            self.esc_hint,
            self.start_hint,
            self.display_mode,
            self.windowed,
            self.borderless,
            self.fullscreen,
            self.resolution,
            self.resolution_note,
            self.language,
            self.lang_uk,
            self.lang_en,
            self.sound,
            self.on,
            self.off,
            self.esc_back,
            self.wave,
            self.score,
            self.best,
            self.kills,
            self.combo,
            self.hp,
            self.dash,
            self.reloading,
            self.sound_off,
            self.hint,
            self.rifle,
            self.shotgun,
            self.paused,
            self.died,
            self.new_record,
            self.elite,
            self.cleared,
            self.bonus,
            self.ammo,
            self.heal,
            self.esc_resume,
            self.esc_menu,
        ];
        for (keys, action) in self.controls.iter() {
            all.push(keys);
            all.push(action);
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font;

    #[test]
    fn every_localised_character_has_a_glyph() {
        for lang in [Lang::Uk, Lang::En] {
            for line in text(lang).all() {
                for ch in line.chars() {
                    if ch == ' ' {
                        continue;
                    }
                    assert!(
                        font::glyph(ch).iter().any(|row| *row != 0),
                        "no glyph for {ch:?} in {line:?} ({lang:?})"
                    );
                }
            }
        }
        for resolution in crate::settings::Resolution::ALL {
            for ch in resolution.label().chars() {
                assert!(
                    font::glyph(ch.to_uppercase().next().unwrap_or(ch))
                        .iter()
                        .any(|row| *row != 0),
                    "no glyph for {ch:?} in a resolution label"
                );
            }
        }
    }

    #[test]
    fn language_codes_round_trip() {
        for lang in [Lang::Uk, Lang::En] {
            assert_eq!(Lang::parse(lang.code()), Some(lang));
        }
    }
}
