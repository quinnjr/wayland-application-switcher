use crate::backend::{WindowBackend, WindowInfo};
use std::collections::HashMap;
use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

const DEST: &str = "org.kde.KWin";
const PATH: &str = "/WindowsRunner";
const IFACE: &str = "org.kde.krunner1";

pub struct KWinBackend {
    conn: Connection,
}

impl KWinBackend {
    pub fn connect() -> anyhow::Result<Self> {
        let conn = Connection::session()?;
        conn.call_method(Some(DEST), "/", Some("org.freedesktop.DBus.Peer"), "Ping", &())
            .map_err(|e| anyhow::anyhow!("KWin not reachable on session bus: {e}"))?;
        Ok(Self { conn })
    }
}

type MatchTuple = (String, String, String, i32, f64, HashMap<String, OwnedValue>);

impl WindowBackend for KWinBackend {
    fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>> {
        let reply = self
            .conn
            .call_method(Some(DEST), PATH, Some(IFACE), "Match", &(query,))?;
        let matches: Vec<MatchTuple> = reply.body().deserialize()?;
        Ok(matches
            .into_iter()
            .map(|(id, title, icon, _category, _relevance, properties)| {
                let subtext = properties
                    .get("subtext")
                    .and_then(|v| <&str>::try_from(v).ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                WindowInfo { id, title, icon, subtext }
            })
            .collect())
    }

    fn activate(&self, id: &str) -> anyhow::Result<()> {
        self.conn
            .call_method(Some(DEST), PATH, Some(IFACE), "Run", &(id, ""))?;
        Ok(())
    }
}
