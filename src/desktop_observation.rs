//! Diagnostic settling analysis. Image time and observed input readiness differ.
use crate::observation::{Bounds, Frame, Rejected as PixelRejected};

#[derive(Clone, Copy, Debug)]
pub struct DesktopObservation {
    pub observed: Frame,
    pub image_ns: Option<u128>,
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
    before_hash: u64,
    before_list_hash: u64,
    frames: &[DesktopObservation],
    minimum_stable_span_ns: u128,
    maximum_uncertainty_ns: u128,
) -> Result<DesktopBounds, Rejected> {
    let mut previous: Option<&DesktopObservation> = None;
    for frame in frames {
        if frame.protected_content {
            return Err(Rejected::ProtectedContent);
        }
        if frame.accumulated_frames > 1 {
            return Err(Rejected::AccumulatedFrames);
        }
        match frame.image_ns {
            None => {
                if frame.fresh
                    || frame.accumulated_frames != 0
                    || frame.observed.hash != before_hash
                    || frame.observed.list_hash != before_list_hash
                    || previous.is_some_and(|p| p.image_ns.is_some())
                {
                    return Err(Rejected::InconsistentStream);
                }
            }
            Some(image_ns) => {
                if image_ns > frame.observed.ended_ns || frame.accumulated_frames != 1 {
                    return Err(Rejected::InconsistentStream);
                }
                if let Some(prior) = previous.filter(|p| p.image_ns.is_some()) {
                    if frame.fresh {
                        if Some(image_ns) <= prior.image_ns {
                            return Err(Rejected::InconsistentStream);
                        }
                    } else if frame.image_ns != prior.image_ns
                        || frame.observed.hash != prior.observed.hash
                        || frame.observed.list_hash != prior.observed.list_hash
                    {
                        return Err(Rejected::InconsistentStream);
                    }
                } else if !frame.fresh {
                    return Err(Rejected::InconsistentStream);
                }
            }
        }
        previous = Some(frame);
    }
    let observed: Vec<_> = frames.iter().map(|f| f.observed).collect();
    let pixels = crate::observation::settled_bounds(
        before_hash,
        before_list_hash,
        &observed,
        minimum_stable_span_ns,
        maximum_uncertainty_ns,
    )
    .map_err(Rejected::Pixels)?;
    let last = frames.last().ok_or(Rejected::InconsistentStream)?;
    let first_match = frames
        .iter()
        .find(|f| {
            f.observed.ready
                && f.observed.hash == pixels.final_hash
                && f.observed.list_hash == last.observed.list_hash
                && f.image_ns.is_some()
        })
        .ok_or(Rejected::InconsistentStream)?;
    let ready = frames
        .iter()
        .position(|f| f.observed.ready)
        .ok_or(Rejected::InconsistentStream)?;
    Ok(DesktopBounds {
        image_update_ns: first_match.image_ns.ok_or(Rejected::InconsistentStream)?,
        first_ready_lower_ns: if ready == 0 {
            0
        } else {
            frames[ready - 1].observed.began_ns
        },
        first_ready_upper_ns: frames[ready].observed.ended_ns,
        pixels,
    })
}

pub fn rgb_hash(bgra: &[u8]) -> Result<u64, &'static str> {
    let (pixels, remainder) = bgra.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err("Incomplete BGRA pixel");
    }
    let mut hash = 0xcbf29ce484222325_u64;
    for pixel in pixels {
        for &value in &pixel[..3] {
            hash = (hash ^ u64::from(value)).wrapping_mul(0x100000001b3);
        }
    }
    Ok(hash)
}
