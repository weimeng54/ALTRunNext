use crate::{
    native::{make_font, scaled},
    skin, theme, theme_ui,
};
use altrun_probe::preferences::{Font, Preferences};
use std::{path::Path, sync::mpsc};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{Controls::*, WindowsAndMessaging::*},
};

pub const READY: u32 = WM_APP + 2;
type ResultImage = (skin::Request, Result<skin::Bitmap, String>);

pub struct Preview {
    request: Option<skin::Request>,
    bitmap: Option<skin::Bitmap>,
    sender: mpsc::Sender<ResultImage>,
    results: mpsc::Receiver<ResultImage>,
    pub prefs: Preferences,
}

impl Preview {
    pub fn new(prefs: Preferences) -> Self {
        let (sender, results) = mpsc::channel();
        Self {
            request: None,
            bitmap: None,
            sender,
            results,
            prefs,
        }
    }

    pub fn update(&mut self, prefs: Preferences, data: &Path, hwnd: HWND) {
        theme::install(&prefs);
        let request = skin::Request::for_settings(&prefs, data, theme::surface());
        theme::install_chrome(&prefs);
        self.prefs = prefs;
        if request == self.request {
            return;
        }
        self.request = request.clone();
        let Some(request) = request else {
            self.bitmap = None;
            return;
        };
        let sender = self.sender.clone();
        let owner = hwnd as usize;
        std::thread::spawn(move || {
            let result = skin::load(&request);
            if sender.send((request, result)).is_ok() {
                unsafe {
                    PostMessageW(owner as HWND, READY, 0, 0);
                }
            }
        });
    }

    pub fn complete(&mut self) -> Option<String> {
        let mut error = None;
        while let Ok((request, result)) = self.results.try_recv() {
            if self.request.as_ref() != Some(&request) {
                continue;
            }
            match result {
                Ok(bitmap) => self.bitmap = Some(bitmap),
                Err(reason) => error = Some(reason),
            }
        }
        error
    }

    pub fn paint(&self, item: &DRAWITEMSTRUCT, dpi: i32, query: &str) {
        theme::install(&self.prefs);
        let dc = item.hDC;
        let px = |n| scaled(n, dpi);
        let rect = item.rcItem;
        let area = |x, y, w, h| RECT {
            left: rect.left + px(x),
            top: rect.top + px(y),
            right: (rect.left + px(x + w)).min(rect.right),
            bottom: (rect.top + px(y + h)).min(rect.bottom),
        };
        theme_ui::fill(dc, &rect, theme::border());
        let inset = RECT {
            left: rect.left + 1,
            top: rect.top + 1,
            right: rect.right - 1,
            bottom: rect.bottom - 1,
        };
        theme_ui::fill(dc, &inset, theme::surface());
        if let Some(bitmap) = &self.bitmap {
            bitmap.draw(dc, inset, &self.prefs.get("BGCrop"));
        }
        let mut matches: Vec<_> = [
            ("Doubao", "豆包", "Doubao.exe"),
            ("download", "下载目录", "%USERPROFILE%\\Downloads"),
            ("desktop", "桌面", "%USERPROFILE%\\Desktop"),
            ("notepad", "记事本", "notepad.exe"),
        ]
        .into_iter()
        .filter(|(alias, _, _)| {
            let query = query.to_ascii_lowercase();
            let mut remaining = query.chars().peekable();
            for ch in alias.to_ascii_lowercase().chars() {
                if remaining.peek() == Some(&ch) {
                    remaining.next();
                }
            }
            remaining.peek().is_none()
        })
        .collect();
        matches.truncate(self.prefs.number("MaxRows").clamp(1, 4) as usize);
        let title = matches.first().map_or("ALTRun", |row| row.1);
        let draw_font = |key: &str, fallback| {
            Font::parse(&self.prefs.get(key)).ok().map_or_else(
                || make_font(fallback, 400, "Microsoft YaHei UI", dpi),
                |font| make_font(font.points * 4 / 3, font.weight, &font.name, dpi),
            )
        };
        let title_font = draw_font("TitleFont", 14);
        let row_font = draw_font("ListFont", 14);
        let command_font = make_font(12, 400, "Consolas", dpi);
        unsafe {
            let old = SelectObject(dc, title_font);
            theme_ui::line_text(
                dc,
                area(12, 4, 376, 28),
                &crate::localization::tr(title),
                theme::font_colour(&self.prefs.get("TitleFont")),
                0,
            );
            SelectObject(dc, row_font);
            let row_height = self.prefs.number("RowHeight").clamp(18, 32);
            let mut y = 67;
            for (i, (alias, name, _)) in matches.iter().enumerate() {
                theme_ui::fill(
                    dc,
                    &area(9, y, 398, row_height),
                    if i == 0 {
                        theme::selection()
                    } else {
                        theme::paper()
                    },
                );
                if i == 0 {
                    theme_ui::fill(dc, &area(9, y + 3, 2, row_height - 6), theme::accent());
                }
                theme_ui::line_text(
                    dc,
                    area(16, y, 19, row_height),
                    &(i + 1).to_string(),
                    theme::muted(),
                    0,
                );
                theme_ui::line_text(
                    dc,
                    area(41, y, 106, row_height),
                    alias,
                    theme::font_colour(&self.prefs.get("ListFont")),
                    0,
                );
                theme_ui::line_text(
                    dc,
                    area(154, y, 243, row_height),
                    &crate::localization::tr(name),
                    theme::font_colour(&self.prefs.get("ListFont")),
                    0,
                );
                y += row_height;
            }
            if matches.is_empty() {
                theme_ui::fill(dc, &area(9, y, 398, 24), theme::paper());
                theme_ui::line_text(
                    dc,
                    area(16, y, 382, 24),
                    &crate::localization::tr("没有匹配项"),
                    theme::muted(),
                    0,
                );
                y += 24;
            }
            if self.prefs.flag("ShowCommandLine")
                && let Some((_, _, path)) = matches.first()
            {
                SelectObject(dc, command_font);
                theme_ui::fill(dc, &area(9, y + 1, 398, 23), theme::paper());
                theme_ui::line_text(
                    dc,
                    area(16, y + 1, 382, 23),
                    &format!("CMD  {path}"),
                    theme::muted(),
                    0,
                );
            }
            SelectObject(dc, old);
            DeleteObject(title_font);
            DeleteObject(row_font);
            DeleteObject(command_font);
        }
        theme::install_chrome(&self.prefs);
    }
}
