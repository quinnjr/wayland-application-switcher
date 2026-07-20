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
