use crate::backend::kwin::KWinBackend;
use crate::backend::{classify_matches, MatchResolution, WindowBackend};
use crate::timeout::{run_with_timeout, TimeoutError};
use std::collections::HashMap;
use std::time::Duration;
use zbus::blocking::Connection;
use zbus::zvariant::Value;
use zbus::MatchRule;

// Effectively unbounded — this only bounds the wait so a `was notify`
// process invoked on a notification nobody ever acts on eventually exits
// instead of blocking forever (see design spec's Notify flow section).
const NOTIFY_WAIT_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

pub enum NotifyEvent {
    ActionInvoked { id: u32, action_key: String },
    Closed { id: u32 },
}

pub fn should_activate(target_id: u32, event: NotifyEvent) -> Option<bool> {
    match event {
        NotifyEvent::ActionInvoked { id, action_key } if id == target_id && action_key == "default" => {
            Some(true)
        }
        NotifyEvent::Closed { id } if id == target_id => Some(false),
        _ => None,
    }
}

fn wait_for_decision(conn: &Connection, notification_id: u32) -> anyhow::Result<bool> {
    let conn = conn.clone();
    match run_with_timeout(NOTIFY_WAIT_TIMEOUT, move || wait_loop(&conn, notification_id)) {
        Ok(result) => result,
        Err(TimeoutError::Elapsed) => Ok(false),
        Err(TimeoutError::WorkerPanicked) => {
            anyhow::bail!("notification listener thread terminated unexpectedly")
        }
    }
}

fn wait_loop(conn: &Connection, notification_id: u32) -> anyhow::Result<bool> {
    // A single match rule on the interface catches both signals; the loop
    // below tells them apart by checking each message's member name.
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.Notifications")?
        .path("/org/freedesktop/Notifications")?
        .build();
    let mut iter = zbus::blocking::MessageIterator::for_match_rule(rule, conn, Some(16))?;

    loop {
        let Some(msg) = iter.next() else { return Ok(false) };
        let msg = msg?;
        let header = msg.header();
        let Some(member) = header.member() else { continue };
        let event = match member.as_str() {
            "ActionInvoked" => {
                let (id, action_key): (u32, String) = msg.body().deserialize()?;
                NotifyEvent::ActionInvoked { id, action_key }
            }
            "NotificationClosed" => {
                let (id, _reason): (u32, u32) = msg.body().deserialize()?;
                NotifyEvent::Closed { id }
            }
            _ => continue,
        };
        if let Some(decision) = should_activate(notification_id, event) {
            return Ok(decision);
        }
    }
}

pub fn run_notify(
    query: &str,
    summary: &str,
    body: Option<&str>,
    icon: Option<&str>,
) -> anyhow::Result<()> {
    let conn = Connection::session()?;

    let actions: Vec<&str> = vec!["default", ""];
    let hints: HashMap<&str, Value> = HashMap::new();

    let reply = conn.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "Notify",
        &(
            "was",
            0u32,
            icon.unwrap_or(""),
            summary,
            body.unwrap_or(""),
            actions,
            hints,
            -1i32,
        ),
    )?;
    let notification_id: u32 = reply.body().deserialize()?;

    let activate = wait_for_decision(&conn, notification_id)?;

    if !activate {
        return Ok(());
    }

    let backend = KWinBackend::from_connection(conn);
    activate_or_report(&backend, query)
}

fn activate_or_report(backend: &dyn WindowBackend, query: &str) -> anyhow::Result<()> {
    let matches = backend.list_windows(query)?;
    match classify_matches(matches) {
        MatchResolution::None => anyhow::bail!("no windows matched \"{query}\""),
        MatchResolution::One(w) => backend.activate(&w.id)?,
        MatchResolution::Ambiguous(matches) => {
            eprintln!("was: \"{query}\" is ambiguous, not activating anything:");
            for w in &matches {
                eprintln!("  {}\t{}", w.id, w.title);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_action_on_target_id_activates() {
        let event = NotifyEvent::ActionInvoked { id: 42, action_key: "default".to_string() };
        assert_eq!(should_activate(42, event), Some(true));
    }

    #[test]
    fn non_default_action_on_target_id_is_ignored() {
        let event = NotifyEvent::ActionInvoked { id: 42, action_key: "other".to_string() };
        assert_eq!(should_activate(42, event), None);
    }

    #[test]
    fn action_on_different_id_is_ignored() {
        let event = NotifyEvent::ActionInvoked { id: 7, action_key: "default".to_string() };
        assert_eq!(should_activate(42, event), None);
    }

    #[test]
    fn closed_on_target_id_exits_quietly() {
        let event = NotifyEvent::Closed { id: 42 };
        assert_eq!(should_activate(42, event), Some(false));
    }

    #[test]
    fn closed_on_different_id_is_ignored() {
        let event = NotifyEvent::Closed { id: 7 };
        assert_eq!(should_activate(42, event), None);
    }

    use crate::backend::WindowInfo;
    use std::cell::RefCell;

    struct FakeBackend {
        windows: Vec<WindowInfo>,
        activated: RefCell<Vec<String>>,
    }

    impl WindowBackend for FakeBackend {
        fn list_windows(&self, _query: &str) -> anyhow::Result<Vec<WindowInfo>> {
            Ok(self.windows.clone())
        }
        fn activate(&self, id: &str) -> anyhow::Result<()> {
            self.activated.borrow_mut().push(id.to_string());
            Ok(())
        }
    }

    fn window(id: &str) -> WindowInfo {
        WindowInfo { id: id.to_string(), title: format!("Window {id}"), icon: String::new(), subtext: String::new() }
    }

    #[test]
    fn zero_matches_errors_and_does_not_activate() {
        let backend = FakeBackend { windows: vec![], activated: RefCell::new(vec![]) };
        let err = activate_or_report(&backend, "query").unwrap_err();
        assert!(err.to_string().contains("query"));
        assert!(backend.activated.borrow().is_empty());
    }

    #[test]
    fn single_match_activates_it() {
        let backend = FakeBackend { windows: vec![window("1")], activated: RefCell::new(vec![]) };
        activate_or_report(&backend, "query").unwrap();
        assert_eq!(*backend.activated.borrow(), vec!["1".to_string()]);
    }

    #[test]
    fn ambiguous_matches_do_not_activate_anything() {
        let backend = FakeBackend {
            windows: vec![window("1"), window("2")],
            activated: RefCell::new(vec![]),
        };
        activate_or_report(&backend, "query").unwrap();
        assert!(backend.activated.borrow().is_empty());
    }
}
