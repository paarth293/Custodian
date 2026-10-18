//! Stop invalidates request authority without blocking independent chat turns.
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
#[derive(Clone)]
struct Binding {
    epoch: Arc<AtomicU64>,
    captured: u64,
}
thread_local! { static ACTIVE: RefCell<Option<Binding>> = const { RefCell::new(None) }; }
pub fn is_current() -> bool {
    ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .is_none_or(|b| b.epoch.load(Ordering::Acquire) == b.captured)
    })
}
pub fn with_epoch<T>(epoch: Arc<AtomicU64>, captured: u64, work: impl FnOnce() -> T) -> T {
    struct Reset(Option<Binding>);
    impl Drop for Reset {
        fn drop(&mut self) {
            ACTIVE.with(|active| *active.borrow_mut() = self.0.take());
        }
    }
    let old = ACTIVE.with(|active| {
        let captured = active
            .borrow()
            .as_ref()
            .filter(|b| Arc::ptr_eq(&b.epoch, &epoch))
            .map_or(captured, |b| b.captured);
        active.replace(Some(Binding { epoch, captured }))
    });
    let _reset = Reset(old);
    work()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn later_request_cannot_revive_a_stopped_worker() {
        let epoch = Arc::new(AtomicU64::new(0));
        with_epoch(epoch.clone(), 0, || {
            assert!(is_current());
            epoch.fetch_add(1, Ordering::AcqRel);
            assert!(!is_current());
            with_epoch(epoch.clone(), 1, || assert!(!is_current()));
        });
        with_epoch(epoch, 1, || assert!(is_current()));
    }
}
