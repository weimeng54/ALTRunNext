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
