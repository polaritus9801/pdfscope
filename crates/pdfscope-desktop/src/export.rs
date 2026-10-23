//! Writing a conversation to a Markdown file.

use std::path::{Path, PathBuf};

use tauri::Manager;

/// A file name that survives every platform, derived from the conversation title.
pub(crate) fn safe_filename(title: &str) -> String {
    let mut name: String = title
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            character if character.is_control() => '-',
            character => character,
        })
        .collect();

    // Runs of replaced characters are one dash, not three.
    name = name
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    name = name.trim_matches([' ', '.']).to_owned();
    name = name.chars().take(80).collect();
    if name.is_empty() {
        name = "conversation".to_owned();
    }
    if !name.to_lowercase().ends_with(".md") {
        name.push_str(".md");
    }
    name
}

/// A path that does not overwrite an earlier export: `name.md`, `name (1).md`, …
pub(crate) fn unique_path(directory: &Path, filename: &str) -> PathBuf {
    let path = directory.join(filename);
    if !path.exists() {
        return path;
    }

    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("conversation");
    for serial in 1.. {
        let candidate = directory.join(format!("{stem} ({serial}).md"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

/// Saves Markdown to the Downloads directory and returns the full path.
#[tauri::command]
pub(crate) fn save_export(
    app: tauri::AppHandle,
    filename: String,
    content: String,
) -> Result<String, String> {
    let directory = app
        .path()
        .download_dir()
        .or_else(|_| app.path().document_dir())
        .map_err(|error| format!("无法定位下载目录：{error}"))?;
    std::fs::create_dir_all(&directory).map_err(|error| format!("无法创建下载目录：{error}"))?;

    let path = unique_path(&directory, &safe_filename(&filename));
    std::fs::write(&path, content).map_err(|error| format!("无法写入文件：{error}"))?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_characters_become_dashes() {
        assert_eq!(
            safe_filename("a/b:c*d?\"e<f>g|h\\i"),
            "a-b-c-d-e-f-g-h-i.md"
        );
    }

    #[test]
    fn empty_titles_fall_back_and_get_an_extension() {
        assert_eq!(safe_filename("   "), "conversation.md");
        assert_eq!(safe_filename("对话"), "对话.md");
    }

    #[test]
    fn long_titles_are_cut() {
        let name = safe_filename(&"字".repeat(120));
        assert_eq!(name.chars().count(), 80 + 3);
    }

    #[test]
    fn unique_paths_never_overwrite() {
        let directory =
            std::env::temp_dir().join(format!("pdfscope-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let first = unique_path(&directory, "chat.md");
        std::fs::write(&first, "x").unwrap();
        let second = unique_path(&directory, "chat.md");
        assert_ne!(first, second);
        assert!(second.ends_with("chat (1).md"));
        std::fs::remove_dir_all(&directory).ok();
    }
}
