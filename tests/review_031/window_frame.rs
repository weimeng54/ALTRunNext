// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

pub(super) fn check(dialog: HWND, owner: HWND, theme: &str) {
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
        let mut bounds = RECT::default();
        GetWindowRect(dialog, &mut bounds);
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let positions = [
            (width / 2, 0),
            (0, height / 2),
            (width / 2, height - 1),
            (width - 1, height / 2),
            (5, height / 2),
        ];
        // Read the visible window as presented, before requesting a repaint.
        // Checking the opposing sides catches both a native bevel and a region
        // clipped to the client dimensions.
        let expected = [edge, edge, edge, edge, body];
        let mut previous = None;
        super::wait(|| {
            let dc = GetWindowDC(dialog);
            let pixels = positions.map(|(x, y)| GetPixel(dc, x, y));
            ReleaseDC(dialog, dc);
            if pixels != expected && previous != Some(pixels) {
                eprintln!(
                    "Window edge/body pixels in {theme}: {pixels:x?}, expected {expected:x?}"
                );
                previous = Some(pixels);
            }
            pixels == expected
        });
    }
}
