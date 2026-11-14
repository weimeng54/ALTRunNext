use crate::{
    native::{make_font, scaled},
    settings_layout::{BoxRect, PREVIEW},
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
    pub selected: usize,
    pub input_colours: (u32, u32, u32),
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
            selected: 0,
            input_colours: (theme::surface(), theme::paper(), theme::text()),
        }
    }

    pub fn update(&mut self, prefs: Preferences, data: &Path, hwnd: HWND) {
        theme::install(&prefs);
        self.input_colours = (
            theme::surface(),
            theme::keyword(),
            theme::font_colour(&prefs.get("KeywordFont")),
        );
        let request = skin::Request::for_settings(&prefs, data, theme::surface());
        theme::install_chrome(&prefs);
        self.prefs = prefs;
        if request == self.request {
            return;
        }
        if let (Some(before), Some(after)) = (&self.request, &request)
            && !before.same_tint(after)
        {
            self.bitmap = None;
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

    pub fn paint_thumb(&self, item: &DRAWITEMSTRUCT, dpi: i32) {
        let rect = item.rcItem;
        theme_ui::fill(item.hDC, &rect, theme::control_surface());
        theme_ui::rounded(
            item.hDC,
            rect,
            scaled(8, dpi),
            theme::selection(),
            theme::border(),
        );
        if let Some(bitmap) = &self.bitmap {
            let inner = RECT {
                left: rect.left + 1,
                top: rect.top + 1,
                right: rect.right - 1,
                bottom: rect.bottom - 1,
            };
            bitmap.draw(item.hDC, inner, "center");
        }
    }

    pub fn matches(&self, query: &str) -> Vec<(&'static str, &'static str, &'static str)> {
        let query = query.to_ascii_lowercase();
        let mut results: Vec<_> = [
            ("Doubao", "ui.doubao", r"D:\Program Files\Doubao\Doubao.exe"),
            ("download", "ui.downloads", r"%USERPROFILE%\Downloads"),
            ("desktop", "ui.desktop", r"%USERPROFILE%\Desktop"),
            ("notepad", "ui.notepad", "notepad.exe"),
            ("calc", "ui.calculator", "calc.exe"),
        ]
        .into_iter()
        .filter_map(|entry| {
            let alias = entry.0.to_ascii_lowercase();
            let rank = if query.is_empty() || alias == query {
                0
            } else if alias.starts_with(&query) {
                1
            } else if alias.contains(&query) {
                2
            } else {
                let mut remain = query.chars().peekable();
                for ch in alias.chars() {
                    if remain.peek() == Some(&ch) {
                        remain.next();
                    }
                }
                if remain.peek().is_some() {
                    return None;
                }
                3
            };
            Some((rank, entry))
        })
        .collect();
        results.sort_by_key(|(rank, _)| *rank);
        results
            .into_iter()
            .map(|(_, entry)| entry)
            .take(self.prefs.number("MaxRows").clamp(1, 5) as usize)
            .collect()
    }

    pub fn geometry(&self, count: usize) -> (BoxRect, i32, i32) {
        let width = self.prefs.number("FormWidth").clamp(240, PREVIEW.w - 32);
        let input = Font::parse(&self.prefs.get("KeywordFont")).map_or(17, |font| font.size_dip());
        let input_height = (input + 10).clamp(33, 54);
        let command_height = if self.prefs.flag("ShowCommandLine") {
            25
        } else {
            0
        };
        let row_max =
            ((PREVIEW.h - 24 - 46 - input_height - command_height) / count.max(1) as i32).max(18);
        let row_height = self.prefs.number("RowHeight").clamp(18, row_max);
        let height = 35
            + input_height
            + 4
            + if count == 0 {
                30
            } else {
                count as i32 * row_height
            }
            + command_height
            + 8;
        (
            BoxRect::new(
                (PREVIEW.w - width) / 2,
                (PREVIEW.h - height) / 2,
                width,
                height,
            ),
            input_height,
            row_height,
        )
    }

    pub fn paint(&self, item: &DRAWITEMSTRUCT, dpi: i32, query: &str) {
        theme::install(&self.prefs);
        let dc = item.hDC;
        let px = |n| scaled(n, dpi);
        let rect = item.rcItem;
        theme_ui::fill(dc, &rect, theme::surface());
        theme_ui::rounded(dc, rect, px(16), theme::chrome(), theme::divider());
        let matches = self.matches(query);
        let (bounds, input_height, row_height) = self.geometry(matches.len());
        let area = |x, y, w, h| BoxRect::new(bounds.x + x, bounds.y + y, w, h).pixels(dpi);
        let panel = bounds.pixels(dpi);
        let shadow = area(-2, 3, bounds.w + 4, bounds.h + 3);
        theme_ui::rounded(dc, shadow, px(16), theme::chrome(), theme::chrome());
        theme_ui::rounded(dc, panel, px(16), theme::surface(), theme::border());
        if let Some(bitmap) = &self.bitmap {
            unsafe {
                let saved = SaveDC(dc);
                if saved != 0 {
                    let region = CreateRoundRectRgn(
                        panel.left + 1,
                        panel.top + 1,
                        panel.right - 1,
                        panel.bottom - 1,
                        px(15),
                        px(15),
                    );
                    if !region.is_null() {
                        ExtSelectClipRgn(dc, region, RGN_AND);
                        bitmap.draw(dc, panel, &self.prefs.get("BGCrop"));
                        DeleteObject(region);
                    }
                    RestoreDC(dc, saved);
                }
            }
        }
        let selected = matches.get(self.selected).or_else(|| matches.first());
        let title = selected.map_or("ALTRunNext", |row| row.1);
        let draw_font = |key: &str, fallback| {
            Font::parse(&self.prefs.get(key)).ok().map_or_else(
                || make_font(fallback, 400, "Microsoft YaHei UI", dpi),
                |font| make_font(font.size_dip(), font.weight, &font.name, dpi),
            )
        };
        let title_font = draw_font("TitleFont", 14);
        let row_font = draw_font("ListFont", 14);
        let alias_font = Font::parse(&self.prefs.get("ListFont")).ok().map_or_else(
            || make_font(14, 600, "Microsoft YaHei UI", dpi),
            |font| make_font(font.size_dip(), font.weight.max(600), &font.name, dpi),
        );
        let command_font = make_font(12, 400, "Consolas", dpi);
        unsafe {
            let old = SelectObject(dc, title_font);
            theme_ui::line_text(
                dc,
                area(12, 1, bounds.w - 24, 33),
                &crate::localization::tr(title),
                theme::font_colour(&self.prefs.get("TitleFont")),
                0,
            );
            SelectObject(dc, row_font);
            let mut y = 39 + input_height;
            for (i, (alias, name, _)) in matches.iter().enumerate() {
                let active = selected.is_some_and(|row| row.0 == *alias);
                theme_ui::fill(
                    dc,
                    &area(9, y, bounds.w - 18, row_height),
                    if active {
                        theme::selection()
                    } else {
                        theme::control_surface()
                    },
                );
                if active {
                    theme_ui::fill(dc, &area(9, y + 4, 3, row_height - 8), theme::accent());
                }
                theme_ui::line_text(
                    dc,
                    area(16, y, 19, row_height),
                    &(i + 1).to_string(),
                    theme::muted(),
                    0,
                );
                SelectObject(dc, alias_font);
                theme_ui::line_text(
                    dc,
                    area(43, y, (bounds.w - 68) * 3 / 7, row_height),
                    alias,
                    theme::font_colour(&self.prefs.get("ListFont")),
                    0,
                );
                SelectObject(dc, row_font);
                theme_ui::line_text(
                    dc,
                    area(
                        43 + (bounds.w - 68) * 3 / 7,
                        y,
                        (bounds.w - 68) * 4 / 7,
                        row_height,
                    ),
                    &crate::localization::tr(name),
                    theme::font_colour(&self.prefs.get("ListFont")),
                    0,
                );
                y += row_height;
            }
            if matches.is_empty() {
                theme_ui::fill(dc, &area(9, y, bounds.w - 18, 30), theme::control_surface());
                theme_ui::line_text(
                    dc,
                    area(16, y, bounds.w - 32, 30),
                    &crate::localization::tr("ui.no_matches"),
                    theme::muted(),
                    0,
                );
                y += 30;
            }
            if self.prefs.flag("ShowCommandLine") {
                let path = selected.map_or("—", |row| row.2);
                SelectObject(dc, command_font);
                theme_ui::fill(dc, &area(9, y, bounds.w - 18, 1), theme::divider());
                theme_ui::fill(
                    dc,
                    &area(9, y + 1, bounds.w - 18, 24),
                    theme::control_surface(),
                );
                theme_ui::line_text(
                    dc,
                    area(16, y + 1, bounds.w - 32, 24),
                    &format!("{}  {path}", crate::localization::tr("ui.command_label")),
                    theme::muted(),
                    0,
                );
            }
            SelectObject(dc, old);
            DeleteObject(title_font);
            DeleteObject(row_font);
            DeleteObject(alias_font);
            DeleteObject(command_font);
        }
        theme::install_chrome(&self.prefs);
    }
}
