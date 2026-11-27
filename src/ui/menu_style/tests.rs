// Copyright (c) 2026 GPT-6
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use super::*;

#[test]
fn cascade_arrow_is_a_single_shape_inside_each_scaled_row() {
    for dpi in [96, 120, 144, 168, 192, 240] {
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            assert!(!dc.is_null());
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: 540,
                    biHeight: -100,
                    biPlanes: 1,
                    biBitCount: 32,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = null_mut();
            let bitmap =
                CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
            assert!(!bitmap.is_null());
            let previous = SelectObject(dc, bitmap);
            let mut style = PopupStyle::new(dpi).unwrap();

            style.submenu("Sort").unwrap();
            let rect = RECT {
                left: 0,
                top: 0,
                right: scaled(195, dpi),
                bottom: style.row_height,
            };
            let center = POINT {
                x: rect.right - scaled(14, dpi),
                y: rect.bottom / 2,
            };
            assert!(style.draw(&DRAWITEMSTRUCT {
                CtlType: ODT_MENU,
                hDC: dc,
                rcItem: rect,
                itemData: *style.items.keys().next().unwrap(),
                ..Default::default()
            }));
            assert_eq!(PtVisible(dc, rect.right - scaled(5, dpi), center.y), 0);
            assert_ne!(
                PtVisible(dc, rect.right - scaled(5, dpi), rect.bottom + 1),
                0
            );
            assert_ne!(PtVisible(dc, scaled(30, dpi), center.y), 0);
            SelectClipRgn(dc, null_mut());
            let radius = scaled(4, dpi);
            let mut ink_rows = 0;
            for y in center.y - radius..center.y + radius {
                let ink = (center.x - radius..=center.x + radius)
                    .filter(|x| GetPixel(dc, *x, y) == theme::text())
                    .collect::<Vec<_>>();
                if !ink.is_empty() {
                    ink_rows += 1;
                    assert!(
                        ink.windows(2).all(|pair| pair[1] == pair[0] + 1),
                        "{dpi} DPI"
                    );
                }
            }
            assert!(ink_rows >= scaled(6, dpi), "{dpi} DPI");
            assert_eq!(
                GetPixel(dc, rect.right - scaled(5, dpi), center.y),
                theme::control_surface()
            );

            SelectObject(dc, previous);
            DeleteObject(bitmap);
            DeleteDC(dc);
        }
    }
}

#[test]
fn raw_history_names_and_action_shortcut_columns_survive_native_insertion() {
    let mut style = PopupStyle::new(120).unwrap();
    let root = style.root();
    let raw = "arg&中文\tCtrl+C";
    style.raw(1, raw);
    style.insert(root, 2, "Copy\tCtrl+C", MFS_DISABLED, null_mut(), false);
    style.separator(root);
    unsafe {
        assert_eq!(GetMenuItemCount(root), 3);
        for position in 0..3 {
            let mut info = MENUITEMINFOW {
                cbSize: size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_ID | MIIM_DATA | MIIM_FTYPE | MIIM_STATE,
                ..Default::default()
            };
            assert_ne!(GetMenuItemInfoW(root, position, 1, &mut info), 0);
            let item = &style.items[&info.dwItemData];
            let msaa = &*(info.dwItemData as *const MSAAMENUINFO);
            assert_eq!(msaa.dwMSAASignature, MSAA_MENU_SIG as u32);
            let name = String::from_utf16_lossy(std::slice::from_raw_parts(
                msaa.pszWText,
                msaa.cchWText as usize,
            ));
            let mut size = MEASUREITEMSTRUCT {
                CtlType: ODT_MENU,
                itemData: info.dwItemData,
                ..Default::default()
            };
            assert!(style.measure(&mut size));
            match position {
                0 => {
                    assert_eq!(info.wID, 1);
                    assert_eq!(name, raw);
                    assert_eq!(item.label, "arg&中文    Ctrl+C");
                    assert!(item.shortcut.is_empty());
                }
                1 => {
                    assert_eq!(info.wID, 2);
                    assert_ne!(info.fState & MFS_DISABLED, 0);
                    assert_eq!(name, "Copy\tCtrl+C");
                    assert_eq!(item.shortcut, "Ctrl+C");
                    assert!(
                        size.itemWidth as i32
                            >= style.text_width("arg&中文    Ctrl+C")
                                + style.text_width("Ctrl+C")
                                + scaled(57, 120)
                    );
                }
                _ => {
                    assert!(item.rule);
                    assert_eq!(size.itemHeight, scaled(7, 120) as u32);
                }
            }
        }
    }
}
