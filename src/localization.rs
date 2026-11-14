use crate::language_pack::{LanguagePack, language_tag};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    io::Read,
    path::Path,
    sync::{Arc, RwLock},
};

include!(concat!(env!("OUT_DIR"), "/messages.rs"));

#[derive(Default)]
pub struct Catalog {
    external: BTreeMap<String, Arc<LanguagePack>>,
    pub invalid: Vec<String>,
}

impl Catalog {
    pub fn load(directory: &Path) -> Self {
        let mut catalog = Self::default();
        let Ok(entries) = fs::read_dir(directory) else {
            return catalog;
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            })
            .collect();
        files.sort_unstable_by_key(|entry| entry.file_name());
        for entry in files {
            let filename = entry.file_name().to_string_lossy().into_owned();
            let result = (|| {
                if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                    return Err("not a regular file".to_owned());
                }
                let mut input = String::new();
                fs::File::open(entry.path())
                    .and_then(|file| file.take(512 * 1024 + 1).read_to_string(&mut input))
                    .map_err(|error| error.to_string())?;
                if input.len() > 512 * 1024 {
                    return Err("file exceeds 512 KiB".into());
                }
                let mut pack = LanguagePack::parse(&input)?;
                pack.check(
                    MESSAGES.iter().map(|(key, english, _)| (*key, *english)),
                    false,
                )?;
                if matches!(pack.locale.as_str(), "en" | "zh-Hans") {
                    return Err("locale is built in".into());
                }
                pack.strings.retain(|key, _| {
                    MESSAGES
                        .binary_search_by_key(&key.as_str(), |(key, _, _)| *key)
                        .is_ok()
                });
                Ok(pack)
            })();
            match result {
                Ok(pack) if !catalog.external.contains_key(&pack.locale) => {
                    catalog.external.insert(pack.locale.clone(), Arc::new(pack));
                }
                _ => catalog.invalid.push(filename),
            }
        }
        catalog
    }

    pub fn options(&self) -> Vec<(String, String)> {
        let mut options = vec![
            ("system".into(), String::new()),
            ("en".into(), "English".into()),
            ("zh-Hans".into(), "简体中文".into()),
        ];
        options.extend(
            self.external
                .values()
                .map(|pack| (pack.locale.clone(), pack.native_name.clone())),
        );
        options
    }

    fn resolve_tag(&self, tag: &str) -> Option<Language> {
        let tag = language_tag(tag)?;
        if tag == "zh" && !self.external.contains_key("zh") {
            return Some(Language::SimplifiedChinese);
        }
        let mut candidate = tag.as_str();
        loop {
            if let Some(pack) = self.external.get(candidate) {
                return Some(Language::External(Arc::clone(pack)));
            }
            match candidate {
                "en" => return Some(Language::English),
                "zh-Hans" | "zh-CN" | "zh-SG" => return Some(Language::SimplifiedChinese),
                _ => {}
            }
            candidate = candidate.rsplit_once('-')?.0;
        }
    }

    pub fn resolve<'a>(
        &self,
        preference: &str,
        system: impl IntoIterator<Item = &'a str>,
    ) -> Language {
        if preference == "system" {
            system
                .into_iter()
                .find_map(|tag| self.resolve_tag(tag))
                .unwrap_or_default()
        } else {
            match preference {
                "zh-Hans" => Language::SimplifiedChinese,
                code => self.external.get(code).map_or(Language::English, |pack| {
                    Language::External(Arc::clone(pack))
                }),
            }
        }
    }
}

#[derive(Clone, Default)]
pub enum Language {
    #[default]
    English,
    SimplifiedChinese,
    External(Arc<LanguagePack>),
}

impl Language {
    pub fn locale(&self) -> &str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-Hans",
            Self::External(pack) => &pack.locale,
        }
    }

    pub fn text<'a>(&'a self, key: &'a str) -> &'a str {
        let Ok(index) = MESSAGES.binary_search_by_key(&key, |(key, _, _)| *key) else {
            return key;
        };
        let (_, english, chinese) = MESSAGES[index];
        match self {
            Self::English => english,
            Self::SimplifiedChinese => chinese,
            Self::External(pack) => pack.strings.get(key).map_or(english, String::as_str),
        }
    }
}

static PACKS: RwLock<Catalog> = RwLock::new(Catalog {
    external: BTreeMap::new(),
    invalid: Vec::new(),
});
static ACTIVE: RwLock<Language> = RwLock::new(Language::English);
thread_local! { static WINDOW_LANGUAGE: RefCell<Option<Language>> = const { RefCell::new(None) }; }

pub fn refresh_packs() {
    if let Ok(exe) = std::env::current_exe() {
        let catalog = Catalog::load(&exe.with_file_name("languages"));
        *PACKS.write().unwrap() = catalog;
    }
}

fn resolve(preference: &str) -> Language {
    let system = if preference == "system" {
        system_languages()
    } else {
        Vec::new()
    };
    PACKS
        .read()
        .unwrap()
        .resolve(preference, system.iter().map(String::as_str))
}

pub fn set_active(preference: &str) {
    *ACTIVE.write().unwrap() = resolve(preference);
}

pub fn install(preference: &str) {
    WINDOW_LANGUAGE.with(|slot| *slot.borrow_mut() = Some(resolve(preference)));
}

pub fn current() -> Language {
    WINDOW_LANGUAGE
        .with(|slot| slot.borrow().clone())
        .unwrap_or_else(|| ACTIVE.read().unwrap().clone())
}

pub fn install_snapshot(language: Language) {
    WINDOW_LANGUAGE.with(|slot| *slot.borrow_mut() = Some(language));
}

pub fn options() -> (Vec<(String, String)>, Vec<String>) {
    let follow_system = tr("settings.follow_system");
    let catalog = PACKS.read().unwrap();
    let mut choices = catalog.options();
    choices[0].1 = follow_system;
    (choices, catalog.invalid.clone())
}

pub fn tr(key: &str) -> String {
    current().text(key).to_owned()
}

pub fn message(key: &str, args: &[(&str, &str)]) -> String {
    let language = current();
    interpolate(language.text(key), args)
}

fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut tail = template;
    while let Some((before, rest)) = tail.split_once('{') {
        output.push_str(before);
        let Some((name, after)) = rest.split_once('}') else {
            tail = &tail[before.len()..];
            break;
        };
        if let Some((_, value)) = args.iter().find(|(key, _)| *key == name) {
            output.push_str(value);
        } else {
            output.push('{');
            output.push_str(&rest[..=name.len()]);
        }
        tail = after;
    }
    output.push_str(tail);
    output
}

#[cfg(windows)]
fn system_languages() -> Vec<String> {
    use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    let mut count = 0;
    let mut size = 0;
    unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            std::ptr::null_mut(),
            &mut size,
        );
        if !(2..=65536).contains(&size) {
            return Vec::new();
        }
        let mut buffer = vec![0; size as usize];
        if GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            buffer.as_mut_ptr(),
            &mut size,
        ) == 0
        {
            return Vec::new();
        }
        buffer
            .split(|&unit| unit == 0)
            .take_while(|lang| !lang.is_empty())
            .filter_map(|lang| String::from_utf16(lang).ok())
            .collect()
    }
}

#[cfg(not(windows))]
fn system_languages() -> Vec<String> {
    Vec::new()
}
