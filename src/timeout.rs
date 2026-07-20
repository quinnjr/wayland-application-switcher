use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug)]
pub enum TimeoutError {
    Elapsed,
    WorkerPanicked,
}

/// Runs `f` on a worker thread, bounded by `timeout`. If `f` hasn't
/// finished by then, this returns `Err(TimeoutError::Elapsed)` and the
/// worker thread is left running rather than cancelled — every caller in
/// this codebase is a one-shot CLI invocation that reports the error and
/// exits shortly after, which reaps the thread. This wouldn't be safe to
/// reuse as-is in a long-lived process without adding real cancellation.
pub fn run_with_timeout<T: Send + 'static>(
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, TimeoutError> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(timeout) {
        Ok(result) => Ok(result),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(TimeoutError::Elapsed),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(TimeoutError::WorkerPanicked),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_before_timeout_returns_ok() {
        let result = run_with_timeout(Duration::from_secs(1), || 42);
        assert!(matches!(result, Ok(42)));
    }

    #[test]
    fn never_completes_before_timeout_returns_elapsed() {
        let result = run_with_timeout(Duration::from_millis(20), || {
            std::thread::sleep(Duration::from_secs(10));
            42
        });
        assert!(matches!(result, Err(TimeoutError::Elapsed)));
    }

    #[test]
    fn panicking_worker_returns_worker_panicked() {
        let result: Result<(), TimeoutError> = run_with_timeout(Duration::from_millis(200), || {
            panic!("simulated worker panic");
        });
        assert!(matches!(result, Err(TimeoutError::WorkerPanicked)));
    }
}
