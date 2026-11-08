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
    _before_hash: u64,
    _before_list_hash: u64,
    _frames: &[Frame],
    _minimum_stable_span_ns: u128,
    _maximum_gap_ns: u128,
) -> Result<Bounds, Rejected> {
    Err(Rejected::NotSettled)
}
