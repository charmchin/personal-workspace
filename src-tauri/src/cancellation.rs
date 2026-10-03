//! A one-way, wakeable cancellation signal; registering and cancelling cannot lose a wakeup.
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
};

#[derive(Default)]
pub(crate) struct Cancellation {
    cancelled: AtomicBool,
    // No timer or detached task is needed. QuoteJob clears retained task wakers on drop.
    wakers: Mutex<Vec<Waker>>,
}

impl Cancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        let wakers = std::mem::take(&mut *self.wakers.lock().unwrap_or_else(|p| p.into_inner()));
        for waker in wakers {
            waker.wake();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn clear_waiters(&self) {
        self.wakers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }

    pub fn poll_cancelled(&self, cx: &Context<'_>) -> Poll<()> {
        let mut wakers = self.wakers.lock().unwrap_or_else(|p| p.into_inner());
        if self.is_cancelled() {
            return Poll::Ready(());
        }
        if !wakers.iter().any(|waker| waker.will_wake(cx.waker())) {
            wakers.push(cx.waker().clone());
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Barrier, atomic::AtomicUsize},
        task::Wake,
    };

    struct Counter(AtomicUsize);
    impl Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn cancellation_registration_race_never_loses_wakeup() {
        for _ in 0..100 {
            let signal = Arc::new(Cancellation::default());
            let barrier = Arc::new(Barrier::new(2));
            let other = signal.clone();
            let gate = barrier.clone();
            let thread = std::thread::spawn(move || {
                gate.wait();
                other.cancel();
            });
            let counter = Arc::new(Counter(AtomicUsize::new(0)));
            let waker = Waker::from(counter.clone());
            let cx = Context::from_waker(&waker);
            barrier.wait();
            let result = signal.poll_cancelled(&cx);
            thread.join().unwrap();
            assert!(result.is_ready() || counter.0.load(Ordering::SeqCst) > 0);
            assert!(signal.poll_cancelled(&cx).is_ready());
        }
    }

    #[test]
    fn cancellation_repeated_polls_deduplicate_and_clear_waiters() {
        let signal = Cancellation::default();
        let counter = Arc::new(Counter(AtomicUsize::new(0)));
        let waker = Waker::from(counter.clone());
        let cx = Context::from_waker(&waker);
        for _ in 0..100 {
            assert!(signal.poll_cancelled(&cx).is_pending());
        }
        assert_eq!(signal.wakers.lock().unwrap().len(), 1);
        signal.clear_waiters();
        assert!(signal.wakers.lock().unwrap().is_empty());
        signal.cancel();
        assert!(signal.poll_cancelled(&cx).is_ready());
        assert_eq!(counter.0.load(Ordering::SeqCst), 0);
    }
}
