//! Shared native theme; COLORREF values are written through rgb() to avoid BGR mistakes.
pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
pub const SURFACE: u32 = rgb(245, 247, 250);
pub const PAPER: u32 = rgb(255, 255, 255);
pub const TEXT: u32 = rgb(32, 44, 58);
pub const MUTED: u32 = rgb(97, 112, 132);
pub const SELECTION: u32 = rgb(231, 239, 248);
pub const ACCENT: u32 = rgb(49, 95, 136);
pub const BORDER: u32 = rgb(198, 208, 220);
pub const ERROR: u32 = rgb(165, 44, 54);
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
pub fn keyword() -> u32 {
    KEYWORD.with(std::cell::Cell::get)
}
pub fn font_colour(value: &str) -> u32 {
    if CONTRAST.with(std::cell::Cell::get) {
        return text();
    }
    let colour = altrun_probe::preferences::Font::parse(value)
        .map(|f| f.colour)
        .unwrap_or(TEXT);
    if colour == TEXT { text() } else { colour }
}
pub fn install(prefs: &altrun_probe::preferences::Preferences) {
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
    let mut palette = if dark {
        Palette {
            surface: rgb(30, 34, 40),
            paper: rgb(24, 28, 34),
            text: rgb(231, 235, 242),
            muted: rgb(161, 174, 192),
            selection: rgb(46, 65, 87),
            accent: rgb(128, 180, 229),
            border: rgb(73, 83, 98),
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
        if (!dark || value != defaults.get(key))
            && let Ok(value) = altrun_probe::preferences::parse_colour(&value)
        {
            *target = value;
        }
    }
    PALETTE.with(|p| p.set(palette));
    CONTRAST.with(|c| c.set(false));
    let value = prefs.get("KeywordColor");
    KEYWORD.with(|c| {
        c.set(if dark && value == defaults.get("KeywordColor") {
            palette.paper
        } else {
            altrun_probe::preferences::parse_colour(&value).unwrap_or(palette.paper)
        })
    });
}
