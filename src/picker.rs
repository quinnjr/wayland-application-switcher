use crate::backend::WindowInfo;

pub trait Picker {
    fn pick(&self, windows: &[WindowInfo]) -> Option<String>;
}
