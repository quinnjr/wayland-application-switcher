use crate::backend::detect_backend;
use crate::picker::TuiPicker;
use crate::resolve::resolve;

pub fn run_switch(query: &str) -> anyhow::Result<()> {
    let backend = detect_backend()?;
    let picker = TuiPicker;
    resolve(backend.as_ref(), &picker, query)
}
