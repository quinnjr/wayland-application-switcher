use std::sync::mpsc;
use std::time::Duration;

/// Per-call timeout for backend D-Bus calls. The README documents this as
/// a single shared 3-second timeout, so both backends reference this one
/// constant rather than defining their own.
pub const DBUS_CALL_TIMEOUT: Duration = Duration::from_secs(3);

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

/// Wraps a blocking zbus call in `run_with_timeout` and maps every failure
/// mode to a user-facing error. `server` names the D-Bus peer ("KWin",
/// "GNOME Shell") for the timeout/panic messages; `hint` is appended to
/// call errors when there's a likely remedy (e.g. a missing extension).
pub fn call_dbus_with_timeout<T: Send + 'static>(
    timeout: Duration,
    server: &'static str,
    context: &'static str,
    hint: Option<&'static str>,
    f: impl FnOnce() -> zbus::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    match run_with_timeout(timeout, f) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => match hint {
            Some(hint) => anyhow::bail!("{context}: {e} ({hint})"),
            None => anyhow::bail!("{context}: {e}"),
        },
        Err(TimeoutError::Elapsed) => anyhow::bail!("{server} is not responding"),
        Err(TimeoutError::WorkerPanicked) => {
            anyhow::bail!("{server} worker thread panicked or exited unexpectedly")
        }
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

    #[test]
    fn dbus_success_returns_the_inner_value() {
        let v = call_dbus_with_timeout(
            Duration::from_secs(1),
            "TestServer",
            "Test call failed",
            None,
            || Ok::<_, zbus::Error>(7u32),
        )
        .unwrap();
        assert_eq!(v, 7);
    }

    #[test]
    fn dbus_error_includes_context_and_hint() {
        let err = call_dbus_with_timeout::<()>(
            Duration::from_secs(1),
            "TestServer",
            "Test call failed",
            Some("install the thing"),
            || Err(zbus::Error::Failure("boom".to_string())),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Test call failed"));
        assert!(msg.contains("boom"));
        assert!(msg.contains("install the thing"));
    }

    #[test]
    fn dbus_error_without_hint_omits_parenthetical() {
        let err = call_dbus_with_timeout::<()>(
            Duration::from_secs(1),
            "TestServer",
            "Test call failed",
            None,
            || Err(zbus::Error::Failure("boom".to_string())),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Test call failed: boom");
    }

    #[test]
    fn dbus_worker_panic_names_the_server() {
        let err = call_dbus_with_timeout::<()>(
            Duration::from_millis(200),
            "TestServer",
            "Test call failed",
            None,
            || panic!("simulated worker panic"),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "TestServer worker thread panicked or exited unexpectedly"
        );
    }

    #[test]
    fn dbus_timeout_names_the_server() {
        let err = call_dbus_with_timeout::<()>(
            Duration::from_millis(20),
            "TestServer",
            "Test call failed",
            None,
            || {
                std::thread::sleep(Duration::from_secs(10));
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "TestServer is not responding");
    }
}
