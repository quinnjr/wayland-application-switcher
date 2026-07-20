use crate::backend::detect_backend;

pub fn run_list(query: Option<&str>) -> anyhow::Result<()> {
    let backend = detect_backend()?;
    let windows = backend.list_windows(query.unwrap_or(""))?;
    for w in &windows {
        println!("{}\t{}\t{}", w.id, w.title, w.subtext);
    }
    Ok(())
}
