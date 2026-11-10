use altrun_probe::desktop_frame::{DesktopFrame, Rejected, Stamp};

fn frame() -> DesktopFrame {
    DesktopFrame {
        present_qpc: 20_000,
        acquire_begin_qpc: 19_000,
        acquire_end_qpc: 21_932,
        readback_end_qpc: 71_495,
        accumulated_frames: 1,
        protected_content: false,
    }
}

#[test]
fn image_timestamp_excludes_later_readback_cost() {
    assert_eq!(frame().stamp_after(10_000, 10_000_000), Ok(Stamp {
        elapsed_ns: 1_000_000, acquisition_lag_ns: 193_200, readback_lag_ns: 5_149_500,
    }));
    let mut slow = frame();
    slow.readback_end_qpc = 1_000_000;
    assert_eq!(slow.stamp_after(10_000, 10_000_000).unwrap().elapsed_ns, 1_000_000);
}

#[test]
fn old_images_are_rejected_instead_of_clamped_to_zero() {
    assert_eq!(frame().stamp_after(20_001, 10_000_000), Err(Rejected::PredatesAction));
}

#[test]
fn pointer_only_is_not_a_desktop_update() {
    let mut f = frame(); f.present_qpc = 0; f.accumulated_frames = 0;
    assert_eq!(f.stamp_after(10_000, 10_000_000), Err(Rejected::NoDesktopUpdate));
}

#[test]
fn accumulated_and_protected_images_cannot_establish_response() {
    let mut f = frame(); f.accumulated_frames = 3;
    assert_eq!(f.stamp_after(10_000, 10_000_000), Err(Rejected::AccumulatedFrames));
    f = frame(); f.protected_content = true;
    assert_eq!(f.stamp_after(10_000, 10_000_000), Err(Rejected::ProtectedContent));
}

#[test]
fn malformed_frequency_and_counter_sequences_are_rejected() {
    for frequency in [0, -1] {
        assert_eq!(frame().stamp_after(10_000, frequency), Err(Rejected::InvalidFrequency));
    }
    for f in [
        DesktopFrame { present_qpc: 22_000, ..frame() },
        DesktopFrame { acquire_begin_qpc: 22_000, ..frame() },
        DesktopFrame { readback_end_qpc: 21_000, ..frame() },
        DesktopFrame { present_qpc: -1, ..frame() },
        DesktopFrame { accumulated_frames: 0, ..frame() },
    ] {
        assert_eq!(f.stamp_after(10_000, 10_000_000), Err(Rejected::InvalidTimeline));
    }
}
