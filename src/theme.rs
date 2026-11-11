//! Shared native theme; COLORREF values are written through rgb() to avoid BGR mistakes.
pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
pub const SURFACE: u32 = rgb(245, 247, 251);
pub const PAPER: u32 = rgb(255, 255, 255);
pub const TEXT: u32 = rgb(23, 37, 53);
pub const MUTED: u32 = rgb(79, 96, 116);
pub const SELECTION: u32 = rgb(220, 234, 250);
pub const ACCENT: u32 = rgb(36, 92, 153);
pub const BORDER: u32 = rgb(118, 137, 158);
pub const ROW: i32 = 22;
pub const LIST_Y: i32 = 65;
pub const BASE_HEIGHT: i32 = 94;

#[derive(Clone, Copy)]
struct Palette {
    surface: u32,
    paper: u32,
    text: u32,
    muted: u32,
    selection: u32,
    accent: u32,
    border: u32,
}
thread_local! {static PALETTE:std::cell::Cell<Palette>=const{std::cell::Cell::new(Palette{surface:SURFACE,paper:PAPER,text:TEXT,muted:MUTED,selection:SELECTION,accent:ACCENT,border:BORDER})};}
pub fn surface() -> u32 {
    PALETTE.with(|p| p.get().surface)
}
pub fn paper() -> u32 {
    PALETTE.with(|p| p.get().paper)
}
pub fn text() -> u32 {
    PALETTE.with(|p| p.get().text)
}
pub fn muted() -> u32 {
    PALETTE.with(|p| p.get().muted)
}
pub fn selection() -> u32 {
    PALETTE.with(|p| p.get().selection)
}
pub fn accent() -> u32 {
    PALETTE.with(|p| p.get().accent)
}
pub fn border() -> u32 {
    PALETTE.with(|p| p.get().border)
}
thread_local! {static KEYWORD:std::cell::Cell<u32>=const{std::cell::Cell::new(PAPER)};static CONTRAST:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
thread_local! {static DARK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };}
pub fn keyword() -> u32 {
    KEYWORD.with(std::cell::Cell::get)
}
pub fn font_colour(value: &str) -> u32 {
    if CONTRAST.with(std::cell::Cell::get) {
        return text();
    }
    let colour = altrun_probe::preferences::Font::parse(value)
        .map(|f| f.colour)
        .unwrap_or(rgb(32, 44, 58));
    if colour == rgb(32, 44, 58) {
        text()
    } else {
        colour
    }
}
pub fn install(prefs: &altrun_probe::preferences::Preferences) {
    install_palette(prefs, true);
}
pub fn install_chrome(prefs: &altrun_probe::preferences::Preferences) {
    install_palette(prefs, false);
}
pub fn high_contrast() -> bool {
    CONTRAST.with(std::cell::Cell::get)
}
pub fn error() -> u32 {
    if high_contrast() {
        text()
    } else if DARK.with(std::cell::Cell::get) {
        rgb(255, 183, 179)
    } else {
        rgb(155, 38, 48)
    }
}
pub fn chrome() -> u32 {
    if high_contrast() {
        surface()
    } else if DARK.with(std::cell::Cell::get) {
        rgb(21, 26, 32)
    } else {
        rgb(236, 240, 246)
    }
}
pub fn control_surface() -> u32 {
    if high_contrast() {
        surface()
    } else if DARK.with(std::cell::Cell::get) {
        rgb(34, 41, 50)
    } else {
        PAPER
    }
}
pub fn divider() -> u32 {
    if high_contrast() {
        border()
    } else if DARK.with(std::cell::Cell::get) {
        rgb(52, 62, 74)
    } else {
        rgb(211, 221, 233)
    }
}
pub fn on_accent() -> u32 {
    if high_contrast() {
        selection()
    } else if DARK.with(std::cell::Cell::get) {
        rgb(19, 41, 65)
    } else {
        PAPER
    }
}
pub fn frame(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Graphics::Dwm::*;
    let dark = u32::from(DARK.with(std::cell::Cell::get) && !high_contrast());
    let caption = if high_contrast() { u32::MAX } else { chrome() };
    let ink = if high_contrast() { u32::MAX } else { text() };
    for (attribute, value) in [(20, dark), (35, caption), (36, ink)] {
        unsafe { DwmSetWindowAttribute(hwnd, attribute, &value as *const _ as _, 4) };
    }
}
fn install_palette(prefs: &altrun_probe::preferences::Preferences, custom: bool) {
    use windows_sys::Win32::{Graphics::Gdi::*, System::Registry::*, UI::WindowsAndMessaging::*};
    #[repr(C)]
    struct Contrast {
        size: u32,
        flags: u32,
        scheme: *mut u16,
    }
    let mut contrast = Contrast {
        size: std::mem::size_of::<Contrast>() as u32,
        flags: 0,
        scheme: std::ptr::null_mut(),
    };
    unsafe {
        SystemParametersInfoW(0x42, contrast.size, &mut contrast as *mut _ as _, 0);
    }
    if contrast.flags & 1 != 0 {
        DARK.with(|c| c.set(false));
        CONTRAST.with(|c| c.set(true));
        KEYWORD.with(|c| c.set(unsafe { GetSysColor(COLOR_WINDOW) }));
        unsafe {
            PALETTE.with(|p| {
                p.set(Palette {
                    surface: GetSysColor(COLOR_WINDOW),
                    paper: GetSysColor(COLOR_WINDOW),
                    text: GetSysColor(COLOR_WINDOWTEXT),
                    muted: GetSysColor(COLOR_WINDOWTEXT),
                    selection: GetSysColor(COLOR_HIGHLIGHT),
                    accent: GetSysColor(COLOR_HIGHLIGHTTEXT),
                    border: GetSysColor(COLOR_WINDOWTEXT),
                })
            });
        }
        return;
    }
    let mut dark = prefs.get("Theme") == "dark";
    if prefs.get("Theme") == "system" {
        unsafe {
            let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let key = "AppsUseLightTheme"
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let mut value = 1u32;
            let mut size = 4;
            RegGetValueW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                key.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                &mut value as *mut _ as _,
                &mut size,
            );
            dark = value == 0;
        }
    }
    DARK.with(|c| c.set(dark));
    let mut palette = if dark {
        Palette {
            surface: rgb(25, 30, 37),
            paper: rgb(23, 29, 36),
            text: rgb(240, 244, 248),
            muted: rgb(180, 192, 206),
            selection: rgb(40, 61, 85),
            accent: rgb(154, 199, 255),
            border: rgb(101, 119, 138),
        }
    } else {
        Palette {
            surface: SURFACE,
            paper: PAPER,
            text: TEXT,
            muted: MUTED,
            selection: SELECTION,
            accent: ACCENT,
            border: BORDER,
        }
    };
    let defaults = altrun_probe::preferences::Preferences::default();
    for (key, target) in [
        ("TitleColor", &mut palette.surface),
        ("ListColor", &mut palette.paper),
        ("TextColor", &mut palette.text),
        ("SelectionColor", &mut palette.selection),
    ] {
        let value = prefs.get(key);
        if custom
            && (key != "TextColor" || prefs.flag("UseCustomTextColor"))
            && value != defaults.get(key)
            && let Ok(value) = altrun_probe::preferences::parse_colour(&value)
        {
            *target = value;
        }
    }
    PALETTE.with(|p| p.set(palette));
    CONTRAST.with(|c| c.set(false));
    let value = prefs.get("KeywordColor");
    KEYWORD.with(|c| {
        c.set(if !custom || value == defaults.get("KeywordColor") {
            palette.paper
        } else {
            altrun_probe::preferences::parse_colour(&value).unwrap_or(palette.paper)
        })
    });
}
