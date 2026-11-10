//! Desktop image update time is separate from CPU readback and hardware display.

#[derive(Clone, Copy, Debug)]
pub struct DesktopFrame {
    pub present_qpc: i64,
    pub acquire_begin_qpc: i64,
    pub acquire_end_qpc: i64,
    pub readback_end_qpc: i64,
    pub accumulated_frames: u32,
    pub protected_content: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Stamp {
    pub elapsed_ns: u128,
    pub acquisition_lag_ns: u128,
    pub readback_lag_ns: u128,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rejected {
    InvalidFrequency,
    InvalidTimeline,
    NoDesktopUpdate,
    PredatesAction,
    AccumulatedFrames,
    ProtectedContent,
}

impl DesktopFrame {
    pub fn stamp_after(&self, _action_qpc: i64, _frequency: i64) -> Result<Stamp, Rejected> {
        Err(Rejected::InvalidTimeline)
    }
}
