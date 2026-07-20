#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub subtext: String,
}

pub trait WindowBackend {
    fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>>;
    fn activate(&self, id: &str) -> anyhow::Result<()>;
}

pub mod kwin;

pub fn detect_backend() -> anyhow::Result<Box<dyn WindowBackend>> {
    Ok(Box::new(kwin::KWinBackend::connect()?))
}
