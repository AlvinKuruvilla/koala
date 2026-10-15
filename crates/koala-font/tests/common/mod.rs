//! Font files on this machine, for tests that run over real data.

use std::path::{Path, PathBuf};

/// Where macOS and common Linux distributions keep system fonts.
const FONT_DIRS: &[&str] = &["/System/Library/Fonts", "/Library/Fonts", "/usr/share/fonts"];

/// Every `.ttf`, `.otf` and `.ttc` file under the system font
/// directories, sorted so failures come out in a stable order.
pub fn system_font_files() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending: Vec<PathBuf> = FONT_DIRS.iter().map(PathBuf::from).collect();
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if is_font_file(&path) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["ttf", "otf", "ttc"]
                .iter()
                .any(|font| extension.eq_ignore_ascii_case(font))
        })
}
