// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::native_snapshot;
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

pub(super) fn check(dialog: HWND, owner: HWND, theme: &str, name: &str) {
    let (edge, body) = match theme {
        "dark" => (0x615449, 0x2e2620),
        "light" => (0xccbeb2, 0xfdfbfa),
        _ => unsafe { (GetSysColor(COLOR_WINDOWTEXT), GetSysColor(COLOR_WINDOW)) },
    };
    unsafe {
        assert_eq!(GetWindow(dialog, GW_OWNER), owner);
        assert_eq!(
            GetWindowLongW(dialog, GWL_EXSTYLE) as u32 & WS_EX_DLGMODALFRAME,
            0
        );
        // Include the non-client frame and sample independently of other
        // windows on the desktop. Save the same bitmap that passed the check.
        let expected = [edge, edge, edge, edge, body];
        let mut previous = None;
        super::wait(|| {
            let image = native_snapshot::capture(dialog, Some(PW_RENDERFULLCONTENT));
            let (width, height) = (image.width, image.height);
            let positions = [
                (width / 2, 0),
                (0, height / 2),
                (width / 2, height - 1),
                (width - 1, height / 2),
                (5, height / 2),
            ];
            let pixels = positions.map(|(x, y)| {
                let offset = ((y * width + x) * 4) as usize;
                let [blue, green, red, _]: [u8; 4] =
                    image.bgra[offset..offset + 4].try_into().unwrap();
                u32::from_le_bytes([red, green, blue, 0])
            });
            if pixels == expected {
                image.save(name);
                return true;
            }
            if previous != Some(pixels) {
                eprintln!(
                    "Window edge/body pixels in {theme}: {pixels:x?}, expected {expected:x?}"
                );
                previous = Some(pixels);
            }
            false
        });
    }
}
