use crate::backend::{WindowBackend, WindowInfo};
use crate::timeout::call_dbus_with_timeout;
use std::collections::HashMap;
use std::time::Duration;
use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

const DEST: &str = "org.kde.KWin";
const PATH: &str = "/WindowsRunner";
const IFACE: &str = "org.kde.krunner1";
const CALL_TIMEOUT: Duration = Duration::from_secs(3);

pub struct KWinBackend {
    conn: Connection,
}

impl KWinBackend {
    pub fn connect() -> anyhow::Result<Self> {
        Ok(Self {
            conn: Connection::session()?,
        })
    }
}

fn call_with_timeout<T: Send + 'static>(
    context: &'static str,
    f: impl FnOnce() -> zbus::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    call_dbus_with_timeout(CALL_TIMEOUT, "KWin", context, None, f)
}

type MatchTuple = (
    String,
    String,
    String,
    i32,
    f64,
    HashMap<String, OwnedValue>,
);

impl WindowBackend for KWinBackend {
    fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>> {
        let conn = self.conn.clone();
        let query = query.to_string();
        let reply = call_with_timeout("KWin Match call failed", move || {
            conn.call_method(Some(DEST), PATH, Some(IFACE), "Match", &(query,))
        })?;
        let matches: Vec<MatchTuple> = reply.body().deserialize()?;
        Ok(matches.into_iter().map(window_info_from_match).collect())
    }

    fn activate(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.clone();
        let id = id.to_string();
        call_with_timeout("KWin Run call failed", move || {
            conn.call_method(Some(DEST), PATH, Some(IFACE), "Run", &(id, ""))
        })?;
        Ok(())
    }
}

fn window_info_from_match(m: MatchTuple) -> WindowInfo {
    let (id, title, icon, _category, _relevance, properties) = m;
    let subtext = properties
        .get("subtext")
        .and_then(|v| <&str>::try_from(v).ok())
        .map(|s| s.to_string())
        .unwrap_or_default();
    WindowInfo {
        id,
        title,
        icon,
        subtext,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn match_tuple(properties: HashMap<String, OwnedValue>) -> MatchTuple {
        (
            "id-1".to_string(),
            "Some Window".to_string(),
            "some-icon".to_string(),
            1,
            0.7,
            properties,
        )
    }

    #[test]
    fn subtext_extracted_when_present_and_a_string() {
        let mut properties = HashMap::new();
        properties.insert(
            "subtext".to_string(),
            OwnedValue::try_from(Value::from("Activate running window on Desktop 1")).unwrap(),
        );
        let info = window_info_from_match(match_tuple(properties));
        assert_eq!(info.subtext, "Activate running window on Desktop 1");
        assert_eq!(info.id, "id-1");
        assert_eq!(info.title, "Some Window");
        assert_eq!(info.icon, "some-icon");
    }

    #[test]
    fn subtext_defaults_to_empty_when_key_missing() {
        let info = window_info_from_match(match_tuple(HashMap::new()));
        assert_eq!(info.subtext, "");
    }

    #[test]
    fn subtext_defaults_to_empty_when_value_is_not_a_string() {
        let mut properties = HashMap::new();
        properties.insert(
            "subtext".to_string(),
            OwnedValue::try_from(Value::from(42i32)).unwrap(),
        );
        let info = window_info_from_match(match_tuple(properties));
        assert_eq!(info.subtext, "");
    }
}
