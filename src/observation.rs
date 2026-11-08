//! Offline analysis of a single trial's sampled final-frame run.
//! Sampling bounds are not hardware presentation timestamps.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub began_ns: u128,
    pub ended_ns: u128,
    pub hash: u64,
    pub ready: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Bounds {
    pub lower_ns: u128,
    pub upper_ns: u128,
    pub final_hash: u64,
    pub stable_span_ns: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejected {
    NoFrames,
    InvalidTimeline,
    Interrupted,
    NoVisibleTransition,
    NotSettled,
}

pub fn settled_bounds(
    _before_hash: u64,
    _frames: &[Frame],
    _minimum_stable_span_ns: u128,
) -> Result<Bounds, Rejected> {
    Err(Rejected::NotSettled)
}
