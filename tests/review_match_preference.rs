#![cfg(windows)]
mod review_native_support;
use review_native_support::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
#[test]
fn saved_choice_applies_only_within_the_same_literal_match_priority() {
    let root = std::env::current_dir()
        .unwrap()
        .join("target/review-match-preference");
    std::fs::create_dir_all(&root).unwrap();
    let data = root.join("shortcuts.tsv");
    let rows = altrun_probe::parse_entries(concat!(
        "abab\tFirst\t__missing__.exe\t0\naaaa\tPreferred\t__missing__.exe\t0\n",
        "doubao\tDoubao\t__missing__.exe\t5000\n",
        "db-tools\tPrefix DB\t__missing__.exe\t1000\n",
        "db\tExact DB\t__missing__.exe\t0\n",
    ))
    .unwrap();
    std::fs::write(&data, altrun_probe::library::serialize(&rows).unwrap()).unwrap();
    let mut usage = altrun_probe::usage::Usage::default();
    usage.favourites.insert("a".into(), rows[1].id.clone());
    usage.favourites.insert("db".into(), rows[2].id.clone());
    std::fs::write(data.with_extension("state.tsv"), usage.text()).unwrap();
    std::fs::write(
        data.with_extension("settings.ini"),
        "[Config]\nRememberFavouratMatch=1\n",
    )
    .unwrap();
    let (_process, launcher) = start(&data, Some("--safe-mode"));
    unsafe {
        let edit = GetDlgItem(launcher, 1001);
        let list = GetDlgItem(launcher, 1002);
        let title = GetDlgItem(launcher, 1000);
        set(edit, "a");
        assert_eq!(text(title), "Preferred");
        set(edit, r"^(..)\1$");
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 0);
        set(edit, "db");
        assert_eq!(SendMessageW(list, LB_GETCOUNT, 0, 0), 3);
        assert_eq!(text(title), "Exact DB");
    }
}
