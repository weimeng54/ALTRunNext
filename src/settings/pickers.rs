// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use crate::ui::wide;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{Controls::Dialogs::*, HiDpi::*},
};
pub fn choose(owner: HWND, key: &str, current: &str) -> Option<String> {
    unsafe {
        if key.ends_with("Font") {
            let font = altrun_probe::preferences::Font::parse(current).ok()?;
            let mut definition: LOGFONTW = std::mem::zeroed();
            definition.lfHeight = -font.points * GetDpiForWindow(owner).max(96) as i32 / 72;
            definition.lfWeight = font.weight;
            definition.lfItalic = u8::from(font.italic);
            definition.lfUnderline = u8::from(font.underline);
            definition.lfStrikeOut = u8::from(font.strike_out);
            definition.lfCharSet = font.charset;
            let name = wide(&font.name);
            let n = name.len().saturating_sub(1).min(31);
            definition.lfFaceName[..n].copy_from_slice(&name[..n]);
            let mut dialog: CHOOSEFONTW = std::mem::zeroed();
            dialog.lStructSize = std::mem::size_of_val(&dialog) as u32;
            dialog.hwndOwner = owner;
            dialog.lpLogFont = &mut definition;
            dialog.Flags = CF_INITTOLOGFONTSTRUCT | CF_SCREENFONTS | CF_EFFECTS;
            dialog.rgbColors = font.colour;
            if ChooseFontW(&mut dialog) == 0 {
                return None;
            }
            let name = String::from_utf16_lossy(
                &definition.lfFaceName[..definition
                    .lfFaceName
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(32)],
            );
            let mut styles = Vec::new();
            if definition.lfWeight >= 600 {
                styles.push("fsBold");
            }
            if definition.lfItalic != 0 {
                styles.push("fsItalic");
            }
            if definition.lfUnderline != 0 {
                styles.push("fsUnderline");
            }
            if definition.lfStrikeOut != 0 {
                styles.push("fsStrikeOut");
            }
            return Some(format!(
                "{name}|[{}]|{}|{}|{}",
                styles.join(","),
                (dialog.iPointSize / 10).max(5),
                dialog.rgbColors,
                definition.lfCharSet
            ));
        }
        if key.ends_with("Color") {
            let mut custom = [0u32; 16];
            let mut dialog: CHOOSECOLORW = std::mem::zeroed();
            dialog.lStructSize = std::mem::size_of_val(&dialog) as u32;
            dialog.hwndOwner = owner;
            dialog.rgbResult = altrun_probe::preferences::parse_colour(current).unwrap_or(0);
            dialog.lpCustColors = custom.as_mut_ptr();
            dialog.Flags = CC_FULLOPEN | CC_RGBINIT;
            if ChooseColorW(&mut dialog) == 0 {
                return None;
            }
            let color = dialog.rgbResult;
            return Some(format!(
                "#{:02X}{:02X}{:02X}",
                color & 255,
                (color >> 8) & 255,
                (color >> 16) & 255
            ));
        }
        crate::windows::file_picker::pick_path(owner, false, key == "WorkingDirectory")
            .map(|p| p.to_string_lossy().into_owned())
    }
}
