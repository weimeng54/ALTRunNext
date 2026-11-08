//! Offline analysis of a single trial's sampled final-frame run.
//! Sampling bounds are not hardware presentation timestamps.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub began_ns: u128,
    pub ended_ns: u128,
    pub hash: u64,
    pub list_hash: u64,
    pub ready: bool,
    pub unoccluded: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Bounds {
    pub lower_ns: u128,
    pub upper_ns: u128,
    pub settled_upper_ns: u128,
    pub final_hash: u64,
    pub stable_span_ns: u128,
    pub maximum_gap_ns: u128,
    pub maximum_capture_ns: u128,
    pub maximum_sampling_span_ns: u128,
    pub sample_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejected {
    NoFrames,
    InvalidTimeline,
    Interrupted,
    NoVisibleTransition,
    NoListTransition,
    Occluded,
    SparseSampling,
    NotSettled,
}

pub fn settled_bounds(
    before_hash: u64,
    before_list_hash: u64,
    frames: &[Frame],
    minimum_stable_span_ns: u128,
    maximum_uncertainty_ns: u128,
) -> Result<Bounds, Rejected> {
    let last = frames.last().ok_or(Rejected::NoFrames)?;
    if minimum_stable_span_ns == 0 || maximum_uncertainty_ns == 0 {
        return Err(Rejected::InvalidTimeline);
    }
    let mut previous_end = 0;
    let mut largest_gap = 0;
    let mut largest_capture = 0;
    let mut largest_sampling_span = 0;
    let mut previous_begin = None;
    let mut had_ready = false;
    for frame in frames {
        if frame.ended_ns < frame.began_ns || frame.began_ns < previous_end {
            return Err(Rejected::InvalidTimeline);
        }
        largest_gap = largest_gap.max(frame.began_ns - previous_end);
        largest_capture = largest_capture.max(frame.ended_ns - frame.began_ns);
        let span = frame.ended_ns - previous_begin.unwrap_or(0);
        largest_sampling_span = largest_sampling_span.max(span);
        previous_begin = Some(frame.began_ns);
        previous_end = frame.ended_ns;
        if had_ready && !frame.ready {
            return Err(Rejected::Interrupted);
        }
        if frame.ready && !frame.unoccluded {
            return Err(Rejected::Occluded);
        }
        had_ready |= frame.ready;
    }
    if largest_gap.max(largest_capture).max(largest_sampling_span) > maximum_uncertainty_ns {
        return Err(Rejected::SparseSampling);
    }
    if !last.ready {
        return Err(Rejected::NotSettled);
    }
    if last.hash == before_hash {
        return Err(Rejected::NoVisibleTransition);
    }
    if last.list_hash == before_list_hash {
        return Err(Rejected::NoListTransition);
    }
    let matches_final =
        |frame: &Frame| frame.ready && frame.hash == last.hash && frame.list_hash == last.list_hash;
    let mut stable_start = frames.len() - 1;
    while stable_start > 0 && matches_final(&frames[stable_start - 1]) {
        stable_start -= 1;
    }
    let stable_span_ns = last.began_ns.saturating_sub(frames[stable_start].ended_ns);
    if frames.len() - stable_start < 2 || stable_span_ns < minimum_stable_span_ns {
        return Err(Rejected::NotSettled);
    }
    let first_match = frames
        .iter()
        .position(matches_final)
        .ok_or(Rejected::NotSettled)?;
    Ok(Bounds {
        lower_ns: if first_match == 0 {
            0
        } else {
            frames[first_match - 1].began_ns
        },
        upper_ns: frames[first_match].ended_ns,
        settled_upper_ns: frames[stable_start].ended_ns,
        final_hash: last.hash,
        stable_span_ns,
        maximum_gap_ns: largest_gap,
        maximum_capture_ns: largest_capture,
        maximum_sampling_span_ns: largest_sampling_span,
        sample_count: frames.len(),
    })
}
