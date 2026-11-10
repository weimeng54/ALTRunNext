//! Experimental observer calibration, never launcher acceptance evidence.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
fn self_check() -> bool {
    false
}

fn main() {}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn both_capture_paths_read_changed_rgb_pixels_in_top_down_order() {
        assert!(super::self_check(), "both paths must read the controlled GDI pattern");
    }
}
