use std::{cell::Cell, collections::BTreeMap, sync::LazyLock};
thread_local! {static ENGLISH: Cell<bool> = const { Cell::new(false) };}
static STRINGS: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    include_str!("../assets/english.txt")
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            (
                key.replace("\\t", "\t").replace("\\n", "\n"),
                value.replace("\\t", "\t").replace("\\n", "\n"),
            )
        })
        .collect()
});
pub fn tr(value: &str) -> String {
    if ENGLISH.with(Cell::get)
        && let Some(translated) = STRINGS.get(value)
    {
        translated.clone()
    } else {
        value.to_owned()
    }
}
pub fn install(language: &str) {
    let english = language == "EN";
    if english {
        LazyLock::force(&STRINGS);
    }
    ENGLISH.with(|active| active.set(english));
}
