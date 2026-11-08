use altrun_probe::observation::{Bounds, Frame, Rejected, settled_bounds};

fn frame(began_ns: u128, hash: u64, ready: bool) -> Frame {
    Frame {
        began_ns,
        ended_ns: began_ns + 2,
        hash,
        list_hash: hash,
        ready,
        unoccluded: true,
    }
}

#[test]
fn valid_final_order_can_differ_between_identical_query_trials() {
    for final_hash in [20, 30] {
        let frames = [
            frame(0, 10, true),
            frame(8, final_hash, true),
            frame(70, final_hash, true),
        ];
        assert_eq!(
            settled_bounds(10, 10, &frames, 50, 200),
            Ok(Bounds {
                lower_ns: 0,
                upper_ns: 10,
                settled_upper_ns: 10,
                final_hash,
                stable_span_ns: 60,
                maximum_gap_ns: 60,
                maximum_capture_ns: 2,
                maximum_sampling_span_ns: 64,
                sample_count: 3,
            })
        );
    }
}

#[test]
fn first_matching_observation_and_final_settling_are_distinct() {
    let frames = [
        frame(0, 10, true),
        frame(4, 20, true),
        frame(8, 30, true),
        frame(12, 20, true),
        frame(18, 30, true),
        frame(72, 30, true),
    ];
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 200),
        Ok(Bounds {
            lower_ns: 4,
            upper_ns: 10,
            settled_upper_ns: 20,
            final_hash: 30,
            stable_span_ns: 52,
            maximum_gap_ns: 52,
            maximum_capture_ns: 2,
            maximum_sampling_span_ns: 56,
            sample_count: 6,
        })
    );
}

#[test]
fn an_unchanged_pre_action_picture_cannot_prove_a_response() {
    assert_eq!(
        settled_bounds(10, 10, &[frame(0, 10, true), frame(100, 10, true)], 50, 200),
        Err(Rejected::NoVisibleTransition)
    );
}

#[test]
fn a_late_unsettled_frame_cannot_prove_completion() {
    assert_eq!(
        settled_bounds(10, 10, &[frame(0, 20, true), frame(100, 30, true)], 50, 200),
        Err(Rejected::NotSettled)
    );
}

#[test]
fn losing_readiness_after_it_was_observed_rejects_the_trial() {
    let frames = [
        frame(0, 20, true),
        frame(10, 20, false),
        frame(100, 20, true),
    ];
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 200),
        Err(Rejected::Interrupted)
    );
}

#[test]
fn early_hidden_frames_can_precede_a_valid_wake() {
    let frames = [
        frame(0, 10, false),
        frame(10, 20, true),
        frame(80, 20, true),
    ];
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 200),
        Ok(Bounds {
            lower_ns: 0,
            upper_ns: 12,
            settled_upper_ns: 12,
            final_hash: 20,
            stable_span_ns: 68,
            maximum_gap_ns: 68,
            maximum_capture_ns: 2,
            maximum_sampling_span_ns: 72,
            sample_count: 3,
        })
    );
}

#[test]
fn reversed_capture_intervals_are_invalid() {
    let frames = [frame(20, 20, true), frame(10, 20, true)];
    assert_eq!(
        settled_bounds(10, 10, &frames, 1, 100),
        Err(Rejected::InvalidTimeline)
    );
}

#[test]
fn changing_only_the_command_or_border_does_not_prove_list_presentation() {
    let mut frames = [frame(0, 20, true), frame(100, 20, true)];
    for sample in &mut frames {
        sample.list_hash = 10;
    }
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 200),
        Err(Rejected::NoListTransition)
    );
}

#[test]
fn stable_overlapping_windows_do_not_prove_result_presentation() {
    let mut frames = [frame(0, 20, true), frame(100, 20, true)];
    for sample in &mut frames {
        sample.unoccluded = false;
    }
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 200),
        Err(Rejected::Occluded)
    );
}

#[test]
fn sparse_sampling_is_not_mistaken_for_continuous_stability() {
    let frames = [frame(0, 20, true), frame(1000, 20, true)];
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 200),
        Err(Rejected::SparseSampling)
    );
}

#[test]
fn a_single_long_capture_is_not_a_stable_sequence() {
    let frames = [Frame {
        ended_ns: 100,
        ..frame(0, 20, true)
    }];
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 200),
        Err(Rejected::NotSettled)
    );
}

#[test]
fn a_blind_gap_before_the_first_capture_is_reported_as_sparse() {
    let frames = [frame(200, 20, true), frame(270, 20, true)];
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 200),
        Err(Rejected::SparseSampling)
    );
}

#[test]
fn capture_duration_is_not_counted_as_observed_stability() {
    let frames = [
        Frame {
            ended_ns: 100,
            ..frame(0, 20, true)
        },
        Frame {
            ended_ns: 200,
            ..frame(100, 20, true)
        },
    ];
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 1000),
        Err(Rejected::NotSettled)
    );
}

#[test]
fn adjacent_long_captures_cannot_hide_sampling_uncertainty_as_zero_gap() {
    let frames = [
        Frame {
            ended_ns: 100,
            ..frame(0, 20, true)
        },
        Frame {
            ended_ns: 200,
            ..frame(100, 20, true)
        },
    ];
    assert_eq!(
        settled_bounds(10, 10, &frames, 50, 100),
        Err(Rejected::SparseSampling)
    );
}
