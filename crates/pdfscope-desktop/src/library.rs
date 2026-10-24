//! Native file access for the recent/recommended shelves.
//!
//! The browser file picker never exposes a path, so the desktop shell owns the native picker and the
//! directory scan. Only top-level `.pdf` files are considered; recursive scans of Downloads or a
//! cloud folder are slow and surprising at startup.

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PdfEntry {
    name: String,
    path: String,
    size: u64,
    modified: f64,
}

fn pdf_entry(path: &Path) -> Option<PdfEntry> {
    if !path.is_file() {
        return None;
    }
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
    {
        return None;
    }
    let name = path.file_name()?.to_string_lossy().to_string();
    if name.starts_with('.') {
        return None;
    }
    let metadata = path.metadata().ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    Some(PdfEntry {
        name,
        path: path.to_string_lossy().to_string(),
        size: metadata.len(),
        modified,
    })
}

/// Native picker. Returns the path only; the renderer asks for the bytes separately so large PDFs
/// cross IPC as a raw response instead of a JSON number array.
#[tauri::command]
pub(crate) async fn pick_pdf() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("PDF", &["pdf"])
        .pick_file()
        .await;
    Ok(file.map(|handle| handle.path().to_string_lossy().to_string()))
}

#[tauri::command]
pub(crate) fn read_pdf(path: String) -> Result<tauri::ipc::Response, String> {
    let bytes = std::fs::read(&path).map_err(|error| format!("这个 PDF 无法读取：{error}"))?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Scan the three standard document folders plus the last five directories the reader opened,
/// keeping only direct child PDFs and returning the newest files first.
#[tauri::command]
pub(crate) fn scan_recommended(recent_dirs: Vec<String>, limit: usize) -> Vec<PdfEntry> {
    let mut directories = Vec::<PathBuf>::new();
    for directory in [
        dirs::download_dir(),
        dirs::desktop_dir(),
        dirs::document_dir(),
    ]
    .into_iter()
    .flatten()
    {
        directories.push(directory);
    }
    for directory in recent_dirs.into_iter().take(5) {
        let path = PathBuf::from(directory);
        if path.is_dir() {
            directories.push(path);
        }
    }

    let mut seen_dirs = HashSet::<PathBuf>::new();
    let mut seen_files = HashSet::<PathBuf>::new();
    let mut entries = Vec::<PdfEntry>::new();
    for directory in directories {
        let Ok(canonical) = directory.canonicalize() else {
            continue;
        };
        if !seen_dirs.insert(canonical.clone()) {
            continue;
        }
        let Ok(read_dir) = std::fs::read_dir(&canonical) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !seen_files.insert(path.clone()) {
                continue;
            }
            if let Some(pdf) = pdf_entry(&path) {
                entries.push(pdf);
            }
        }
    }

    entries.sort_by(|left, right| {
        right
            .modified
            .partial_cmp(&left.modified)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    entries.truncate(limit.clamp(1, 40));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scans_direct_pdfs_and_ignores_missing_or_nested_dirs() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pdfscope-recommended-{stamp}"));
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("fixture directory");
        fs::write(root.join("Paper.PDF"), b"%PDF").expect("fixture pdf");
        fs::write(root.join("notes.txt"), b"text").expect("fixture text");
        fs::write(nested.join("Nested.pdf"), b"%PDF").expect("nested pdf");

        let entries = scan_recommended(
            vec![
                root.to_string_lossy().to_string(),
                root.join("missing").to_string_lossy().to_string(),
            ],
            40,
        );
        assert!(entries.iter().any(|entry| entry.name == "Paper.PDF"));
        assert!(!entries.iter().any(|entry| entry.name == "Nested.pdf"));
        assert!(!entries.iter().any(|entry| entry.name == "notes.txt"));

        fs::remove_dir_all(root).ok();
    }
}
