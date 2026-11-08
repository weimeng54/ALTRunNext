use altrun_probe::observation::{Bounds, Frame, Rejected, settled_bounds};

fn frame(began_ns: u128, hash: u64, ready: bool) -> Frame {
    Frame {
        began_ns,
        ended_ns: began_ns + 2,
        hash,
        ready,
    }
}

#[test]
fn valid_final_order_can_differ_between_identical_query_trials() {
    for final_hash in [20, 30] {
        let frames = [frame(0, 10, true), frame(8, final_hash, true), frame(70, final_hash, true)];
        assert_eq!(
            settled_bounds(10, &frames, 50),
            Ok(Bounds {
                lower_ns: 0,
                upper_ns: 10,
                final_hash,
                stable_span_ns: 64,
            })
        );
    }
}

#[test]
fn a_transient_match_before_the_last_stable_run_is_not_accepted() {
    let frames = [
        frame(0, 10, true),
        frame(4, 20, true),
        frame(8, 30, true),
        frame(12, 20, true),
        frame(18, 30, true),
        frame(72, 30, true),
    ];
    assert_eq!(
        settled_bounds(10, &frames, 50),
        Ok(Bounds {
            lower_ns: 12,
            upper_ns: 20,
            final_hash: 30,
            stable_span_ns: 56,
        })
    );
}

#[test]
fn an_unchanged_pre_action_picture_cannot_prove_a_response() {
    assert_eq!(
        settled_bounds(10, &[frame(0, 10, true), frame(100, 10, true)], 50),
        Err(Rejected::NoVisibleTransition)
    );
}

#[test]
fn a_late_unsettled_frame_cannot_prove_completion() {
    assert_eq!(
        settled_bounds(10, &[frame(0, 20, true), frame(100, 30, true)], 50),
        Err(Rejected::NotSettled)
    );
}

#[test]
fn losing_readiness_after_it_was_observed_rejects_the_trial() {
    let frames = [frame(0, 20, true), frame(10, 20, false), frame(100, 20, true)];
    assert_eq!(settled_bounds(10, &frames, 50), Err(Rejected::Interrupted));
}

#[test]
fn early_hidden_frames_can_precede_a_valid_wake() {
    let frames = [frame(0, 10, false), frame(10, 20, true), frame(80, 20, true)];
    assert_eq!(
        settled_bounds(10, &frames, 50),
        Ok(Bounds {
            lower_ns: 0,
            upper_ns: 12,
            final_hash: 20,
            stable_span_ns: 72,
        })
    );
}

#[test]
fn reversed_capture_intervals_are_invalid() {
    let frames = [frame(20, 20, true), frame(10, 20, true)];
    assert_eq!(settled_bounds(10, &frames, 1), Err(Rejected::InvalidTimeline));
}
