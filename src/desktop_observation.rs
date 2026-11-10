//! Diagnostic settling analysis. Image time and observed input readiness differ.
use crate::observation::{Bounds, Frame, Rejected as PixelRejected};

#[derive(Clone, Copy, Debug)]
pub struct DesktopObservation {
    pub observed: Frame,
    pub image_ns: u128,
    pub fresh: bool,
    pub accumulated_frames: u32,
    pub protected_content: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DesktopBounds {
    pub pixels: Bounds,
    pub image_update_ns: u128,
    pub first_ready_lower_ns: u128,
    pub first_ready_upper_ns: u128,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rejected {
    InconsistentStream,
    AccumulatedFrames,
    ProtectedContent,
    Pixels(PixelRejected),
}

pub fn analyze(
    _before_hash: u64,
    _before_list_hash: u64,
    _frames: &[DesktopObservation],
    _minimum_stable_span_ns: u128,
    _maximum_uncertainty_ns: u128,
) -> Result<DesktopBounds, Rejected> {
    Err(Rejected::InconsistentStream)
}

pub fn rgb_hash(_bgra: &[u8]) -> Result<u64, &'static str> {
    Err("RGB hash not implemented")
}
