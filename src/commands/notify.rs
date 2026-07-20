use crate::backend::detect_backend;
use std::collections::HashMap;
use zbus::blocking::Connection;
use zbus::zvariant::Value;
use zbus::MatchRule;

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

    // A single match rule on the interface catches both signals; the loop
    // below tells them apart by checking each message's member name.
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.Notifications")?
        .path("/org/freedesktop/Notifications")?
        .build();
    let mut iter = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, Some(16))?;

    let activate = loop {
        let Some(msg) = iter.next() else { break false };
        let msg = msg?;
        let Some(member) = msg.header().member().map(|m| m.to_string()) else { continue };
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
            break decision;
        }
    };

    if !activate {
        return Ok(());
    }

    let backend = detect_backend()?;
    let matches = backend.list_windows(query)?;
    match matches.len() {
        0 => anyhow::bail!("no windows matched \"{query}\""),
        1 => backend.activate(&matches[0].id)?,
        _ => {
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
}
