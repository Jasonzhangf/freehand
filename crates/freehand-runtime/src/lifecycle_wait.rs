use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const CANCEL_POLL_MILLIS: u64 = 100;

pub(crate) fn sleep_with_cancel(cancel: &AtomicBool, duration: Duration) {
    let poll = Duration::from_millis(CANCEL_POLL_MILLIS);
    let mut remaining = duration;
    while !remaining.is_zero() && !cancel.load(Ordering::Acquire) {
        let step = remaining.min(poll);
        thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
}
