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
    pub fn stamp_after(&self, action_qpc: i64, frequency: i64) -> Result<Stamp, Rejected> {
        if frequency <= 0 {
            return Err(Rejected::InvalidFrequency);
        }
        if self.protected_content {
            return Err(Rejected::ProtectedContent);
        }
        if action_qpc < 0
            || self.present_qpc < 0
            || self.acquire_begin_qpc < 0
            || self.acquire_end_qpc < self.acquire_begin_qpc
            || self.readback_end_qpc < self.acquire_end_qpc
            || self.present_qpc > self.acquire_end_qpc
        {
            return Err(Rejected::InvalidTimeline);
        }
        if self.present_qpc == 0 && self.accumulated_frames == 0 {
            return Err(Rejected::NoDesktopUpdate);
        }
        if self.present_qpc == 0 || self.accumulated_frames == 0 {
            return Err(Rejected::InvalidTimeline);
        }
        if self.accumulated_frames > 1 {
            return Err(Rejected::AccumulatedFrames);
        }
        if self.present_qpc < action_qpc {
            return Err(Rejected::PredatesAction);
        }
        // i64 counters are widened before subtraction and scaling.
        let ns = |end: i64, start: i64| {
            ((i128::from(end) - i128::from(start)) * 1_000_000_000 / i128::from(frequency)) as u128
        };
        Ok(Stamp {
            elapsed_ns: ns(self.present_qpc, action_qpc),
            acquisition_lag_ns: ns(self.acquire_end_qpc, self.present_qpc),
            readback_lag_ns: ns(self.readback_end_qpc, self.present_qpc),
        })
    }
}
