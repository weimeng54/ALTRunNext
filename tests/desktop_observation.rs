use altrun_probe::{
    desktop_observation::{DesktopObservation, Rejected, analyze, rgb_hash},
    observation::{Frame, Rejected as PixelRejected},
};

fn trace() -> Vec<DesktopObservation> {
    [
        (1_100_000, 5_000_000, false, true),
        (5_100_000, 7_000_000, true, false),
        (10_000_000, 12_000_000, true, false),
    ]
    .into_iter()
    .map(|(began_ns, ended_ns, ready, fresh)| DesktopObservation {
        observed: Frame {
            began_ns,
            ended_ns,
            hash: 9,
            list_hash: 8,
            ready,
            unoccluded: true,
        },
        image_ns: Some(1_000_000),
        fresh,
        accumulated_frames: 1,
        protected_content: false,
    })
    .collect()
}

#[test]
fn image_update_and_observed_readiness_are_separate() {
    let bounds = analyze(1, 2, &trace(), 2_000_000, 50_000_000).unwrap();
    assert_eq!(bounds.image_update_ns, 1_000_000);
    assert_eq!(bounds.pixels.upper_ns, 7_000_000);
    assert_eq!(
        (bounds.first_ready_lower_ns, bounds.first_ready_upper_ns),
        (1_100_000, 7_000_000)
    );
}

#[test]
fn a_slow_application_is_not_discarded_as_a_sampling_hole() {
    let mut frames = Vec::new();
    for index in 0..20 {
        frames.push(DesktopObservation {
            observed: Frame {
                began_ns: index * 5_000_000,
                ended_ns: index * 5_000_000 + 1_000_000,
                hash: 1,
                list_hash: 2,
                ready: false,
                unoccluded: false,
            },
            image_ns: None,
            fresh: false,
            accumulated_frames: 0,
            protected_content: false,
        });
    }
    for mut update in trace() {
        update.observed.began_ns += 100_000_000;
        update.observed.ended_ns += 100_000_000;
        update.image_ns = Some(101_000_000);
        frames.push(update);
    }
    assert_eq!(
        analyze(1, 2, &frames, 2_000_000, 50_000_000)
            .unwrap()
            .image_update_ns,
        101_000_000
    );
}

#[test]
fn no_update_cannot_create_new_pixels_or_a_new_image_timestamp() {
    for timestamp_changed in [false, true] {
        let mut frames = trace();
        if timestamp_changed {
            frames[1].image_ns = Some(1_000_001);
        } else {
            frames[1].observed.hash += 1;
        }
        assert_eq!(
            analyze(1, 2, &frames, 2_000_000, 50_000_000),
            Err(Rejected::InconsistentStream)
        );
    }
    let mut frames = trace();
    frames[0].fresh = false;
    assert_eq!(
        analyze(1, 2, &frames, 2_000_000, 50_000_000),
        Err(Rejected::InconsistentStream)
    );
}

#[test]
fn accumulated_and_masked_frames_are_rejected_not_silently_omitted() {
    let mut frames = trace();
    frames[1].accumulated_frames = 3;
    assert_eq!(
        analyze(1, 2, &frames, 2_000_000, 50_000_000),
        Err(Rejected::AccumulatedFrames)
    );
    frames = trace();
    frames[1].protected_content = true;
    assert_eq!(
        analyze(1, 2, &frames, 2_000_000, 50_000_000),
        Err(Rejected::ProtectedContent)
    );
}

#[test]
fn a_fresh_image_cannot_predate_the_last_image_or_arrive_from_the_future() {
    for image_ns in [999_999, 7_000_001] {
        let mut frames = trace();
        frames[1].fresh = true;
        frames[1].image_ns = Some(image_ns);
        assert_eq!(
            analyze(1, 2, &frames, 2_000_000, 50_000_000),
            Err(Rejected::InconsistentStream)
        );
    }
}

#[test]
fn focus_interruptions_and_sampling_holes_remain_rejected() {
    let mut frames = trace();
    frames[2].observed.ready = false;
    assert_eq!(
        analyze(1, 2, &frames, 2_000_000, 50_000_000),
        Err(Rejected::Pixels(PixelRejected::Interrupted))
    );
    frames = trace();
    frames[2].observed.began_ns = 80_000_000;
    frames[2].observed.ended_ns = 82_000_000;
    assert_eq!(
        analyze(1, 2, &frames, 2_000_000, 50_000_000),
        Err(Rejected::Pixels(PixelRejected::SparseSampling))
    );
}

#[test]
fn alpha_differences_are_not_rgb_transitions() {
    assert_eq!(
        rgb_hash(&[1, 2, 3, 0, 4, 5, 6, 0]).unwrap(),
        rgb_hash(&[1, 2, 3, 255, 4, 5, 6, 255]).unwrap()
    );
    assert_ne!(
        rgb_hash(&[1, 2, 3, 0]).unwrap(),
        rgb_hash(&[1, 2, 4, 0]).unwrap()
    );
    assert!(rgb_hash(&[1, 2, 3]).is_err());
}
