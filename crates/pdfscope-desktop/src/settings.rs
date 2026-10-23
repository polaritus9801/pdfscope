//! Where the model configuration lives.
//!
//! The non-secret half is a small JSON file in the app config directory; the key goes to the
//! operating system's keychain and is never written next to the settings, never returned to the
//! webview, and never logged.

use std::collections::BTreeMap;
use std::path::PathBuf;

use futures::StreamExt;
use pdfscope_ai::{ChatRequest, Message, Protocol, ProviderConfig, StreamEvent};
use serde::{Deserialize, Serialize};
use tauri::Manager;

/// The default instructions sent with every question. Editable in settings.
pub(crate) const DEFAULT_SYSTEM_PROMPT: &str = "\
你是一名读者身边的助手，正在陪他读一本 PDF。\
你可以自由回答，但必须分清三类内容：\
1. 书里说了什么：只依据读者提供的摘录和页面，用 [p.页码] 标注来源。\
2. 一般知识：解释术语、补充背景、举例、与别的作者或理论对比，都可以答，但不要给它们编页码，\
   必要时用一句话说明这是书外知识。\
3. 不确定：直说不确定，不要假装书里写过。\
读者问的即使超出这本书的范围也没关系，照常回答；只是别把书外内容说成书里的。\
用读者提问的语言回答；他没有指定语言时用中文。\
先给结论，再给依据，简明扼要。";

/// Where and how big the window was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WindowState {
    /// Outer width in physical pixels.
    pub(crate) width: u32,
    /// Outer height in physical pixels.
    pub(crate) height: u32,
    /// Left edge in physical pixels.
    pub(crate) x: i32,
    /// Top edge in physical pixels.
    pub(crate) y: i32,
    /// Whether the window was maximised.
    pub(crate) maximized: bool,
}

/// The key lives in its own file, readable only by this user, next to the settings.
///
/// The keychain was the first choice, but a prompt on every launch is a worse trade for a local
/// reading tool than a file with 0600 permissions: the threat it defends against — another process
/// reading your files — is not one the keychain removes for the same user either.
const KEY_FILE: &str = "api-key";

/// The search provider key lives beside the model key, with the same permissions.
const SEARCH_KEY_FILE: &str = "search-key";
/// Per-provider search keys, kept in one permission-restricted file.
const SEARCH_KEYS_FILE: &str = "search-keys.json";

/// What is stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Settings {
    /// Display name of the chosen provider.
    pub(crate) preset: String,
    /// `openai` or `anthropic`.
    pub(crate) protocol: String,
    /// Endpoint root.
    pub(crate) base_url: String,
    /// Model identifier.
    pub(crate) model: String,
    /// Instructions sent with every question.
    #[serde(default = "default_prompt")]
    pub(crate) system_prompt: String,
    /// `system`, `light` or `dark`.
    #[serde(default = "default_theme")]
    pub(crate) theme: String,
    /// Upper bound on generated tokens; `None` means no limit.
    #[serde(default)]
    pub(crate) max_tokens: Option<u32>,
    /// Answer text size in pixels.
    #[serde(default = "default_font_size")]
    pub(crate) font_size: u16,
    /// Whether to reopen a document where it was left.
    #[serde(default = "default_true")]
    pub(crate) remember_position: bool,
    /// Zoom a newly opened document starts at, as a percentage of fit width.
    #[serde(default = "default_zoom")]
    pub(crate) default_zoom: u16,
    /// `continuous`, `single` or `spread`.
    #[serde(default = "default_mode")]
    pub(crate) page_mode: String,
    /// Shift the spread pairing by one page.
    #[serde(default)]
    pub(crate) spread_offset: bool,
    /// Most pages one question may carry; zero means the whole document.
    #[serde(default)]
    pub(crate) page_limit: u32,
    /// Per-request limit in seconds; zero waits as long as the provider does.
    #[serde(default)]
    pub(crate) request_timeout: u32,
    /// Search backend id (`deepseek`, `tavily`, `brave`, `exa`, `serper`).
    #[serde(default = "default_search_provider")]
    pub(crate) search_provider: String,
    /// Last window geometry, when the shell has recorded one.
    #[serde(default)]
    pub(crate) window: Option<WindowState>,
}

fn default_zoom() -> u16 {
    100
}

fn default_mode() -> String {
    "continuous".to_owned()
}

fn default_font_size() -> u16 {
    14
}

fn default_true() -> bool {
    true
}

fn default_search_provider() -> String {
    "deepseek".to_owned()
}

fn default_prompt() -> String {
    DEFAULT_SYSTEM_PROMPT.to_owned()
}

fn default_theme() -> String {
    "system".to_owned()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preset: "MiMo".to_owned(),
            protocol: "openai".to_owned(),
            base_url: "https://api.xiaomimimo.com/v1".to_owned(),
            model: "mimo-v2.6-flash".to_owned(),
            system_prompt: default_prompt(),
            theme: default_theme(),
            max_tokens: None,
            font_size: default_font_size(),
            remember_position: true,
            default_zoom: default_zoom(),
            page_mode: default_mode(),
            spread_offset: false,
            page_limit: 0,
            request_timeout: 0,
            search_provider: default_search_provider(),
            window: None,
        }
    }
}

/// What the settings sheet sends.
#[derive(Debug, Deserialize)]
pub(crate) struct SettingsInput {
    /// Display name of the chosen provider.
    pub(crate) preset: String,
    /// `openai` or `anthropic`.
    pub(crate) protocol: String,
    /// Endpoint root.
    pub(crate) base_url: String,
    /// Model identifier.
    pub(crate) model: String,
    /// Instructions sent with every question.
    #[serde(default)]
    pub(crate) system_prompt: Option<String>,
    /// `system`, `light` or `dark`.
    #[serde(default)]
    pub(crate) theme: Option<String>,
    /// Upper bound on generated tokens; absent or null means no limit.
    #[serde(default)]
    pub(crate) max_tokens: Option<u32>,
    /// Answer text size in pixels.
    #[serde(default)]
    pub(crate) font_size: Option<u16>,
    /// Whether to reopen a document where it was left.
    #[serde(default)]
    pub(crate) remember_position: Option<bool>,
    /// Zoom a newly opened document starts at.
    #[serde(default)]
    pub(crate) default_zoom: Option<u16>,
    /// `continuous`, `single` or `spread`.
    #[serde(default)]
    pub(crate) page_mode: Option<String>,
    /// Shift the spread pairing by one page.
    #[serde(default)]
    pub(crate) spread_offset: Option<bool>,
    /// Most pages one question may carry; zero or absent means the whole document.
    #[serde(default)]
    pub(crate) page_limit: Option<u32>,
    /// Per-request limit in seconds; zero or absent means no limit.
    #[serde(default)]
    pub(crate) request_timeout: Option<u32>,
    /// Search backend id.
    #[serde(default)]
    pub(crate) search_provider: Option<String>,
    /// A new key, when the user typed one. `None` keeps the stored key.
    pub(crate) api_key: Option<String>,
    /// A new search key, when the user typed one. `None` keeps the stored key.
    #[serde(default)]
    pub(crate) search_api_key: Option<String>,
}

/// What the webview is allowed to see.
#[derive(Debug, Serialize)]
pub(crate) struct SettingsView {
    /// Display name of the chosen provider.
    pub(crate) preset: String,
    /// `openai` or `anthropic`.
    pub(crate) protocol: String,
    /// Endpoint root.
    pub(crate) base_url: String,
    /// Model identifier.
    pub(crate) model: String,
    /// Instructions sent with every question.
    pub(crate) system_prompt: String,
    /// `system`, `light` or `dark`.
    pub(crate) theme: String,
    /// Upper bound on generated tokens; `None` means no limit.
    pub(crate) max_tokens: Option<u32>,
    /// Answer text size in pixels.
    pub(crate) font_size: u16,
    /// Whether to reopen a document where it was left.
    pub(crate) remember_position: bool,
    /// Zoom a newly opened document starts at.
    pub(crate) default_zoom: u16,
    /// `continuous`, `single` or `spread`.
    pub(crate) page_mode: String,
    /// Shift the spread pairing by one page.
    pub(crate) spread_offset: bool,
    /// Most pages one question may carry; zero means the whole document.
    pub(crate) page_limit: u32,
    /// Per-request limit in seconds; zero waits as long as the provider does.
    pub(crate) request_timeout: u32,
    /// Selected search backend id.
    pub(crate) search_provider: String,
    /// Whether a key is stored. The key itself never crosses this boundary.
    pub(crate) has_key: bool,
    /// Whether a search-provider key is stored.
    pub(crate) has_search_key: bool,
}

/// Reads the last recorded window geometry, if any.
pub(crate) fn remembered_window(app: &tauri::AppHandle) -> Option<WindowState> {
    read_settings(app).window
}

/// Records the window geometry, leaving every other preference untouched.
pub(crate) fn remember_window(app: &tauri::AppHandle, window: WindowState) -> Result<(), String> {
    let mut settings = read_settings(app);
    settings.window = Some(window);
    let path = settings_path(app)?;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("无法序列化设置：{error}"))?;
    std::fs::write(path, json).map_err(|error| format!("无法写入设置：{error}"))?;
    Ok(())
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位配置目录：{error}"))?;
    std::fs::create_dir_all(&directory).map_err(|error| format!("无法创建配置目录：{error}"))?;
    Ok(directory.join("settings.json"))
}

fn read_settings(app: &tauri::AppHandle) -> Settings {
    let Ok(path) = settings_path(app) else {
        return Settings::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn key_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位配置目录：{error}"))?;
    std::fs::create_dir_all(&directory).map_err(|error| format!("无法创建配置目录：{error}"))?;
    Ok(directory.join(KEY_FILE))
}

fn read_key(app: &tauri::AppHandle) -> Option<String> {
    let path = key_path(app).ok()?;
    let key = std::fs::read_to_string(path).ok()?;
    let key = key.trim().to_owned();
    (!key.is_empty()).then_some(key)
}

fn write_key(app: &tauri::AppHandle, key: &str) -> Result<(), String> {
    let path = key_path(app)?;
    std::fs::write(&path, key).map_err(|error| format!("无法写入 API Key：{error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

fn has_key(app: &tauri::AppHandle) -> bool {
    read_key(app).is_some()
}

fn search_keys_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位配置目录：{error}"))?;
    std::fs::create_dir_all(&directory).map_err(|error| format!("无法创建配置目录：{error}"))?;
    Ok(directory.join(SEARCH_KEYS_FILE))
}

fn read_search_keys(app: &tauri::AppHandle) -> BTreeMap<String, String> {
    let mut keys: BTreeMap<String, String> = search_keys_path(app)
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();

    // Migrate the single Tavily key written by the first build.
    if !keys.contains_key("tavily") {
        if let Ok(path) = app
            .path()
            .app_config_dir()
            .map(|directory| directory.join(SEARCH_KEY_FILE))
        {
            if let Ok(key) = std::fs::read_to_string(path) {
                let key = key.trim();
                if !key.is_empty() {
                    keys.insert("tavily".to_owned(), key.to_owned());
                }
            }
        }
    }
    keys
}

fn write_search_key(app: &tauri::AppHandle, provider: &str, key: &str) -> Result<(), String> {
    let mut keys = read_search_keys(app);
    keys.insert(provider.to_owned(), key.to_owned());
    let path = search_keys_path(app)?;
    let json = serde_json::to_string_pretty(&keys)
        .map_err(|error| format!("无法序列化搜索 Key：{error}"))?;
    std::fs::write(&path, json).map_err(|error| format!("无法写入搜索 Key：{error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// The key for one provider: its own, or the model key for DeepSeek.
fn search_key_for(app: &tauri::AppHandle, provider: &str) -> Option<String> {
    let keys = read_search_keys(app);
    keys.get(provider)
        .cloned()
        .or_else(|| (provider == "deepseek").then(|| read_key(app)).flatten())
}

/// Reads the stored configuration for the webview.
#[tauri::command]
pub(crate) fn load_settings(app: tauri::AppHandle) -> Result<SettingsView, String> {
    let settings = read_settings(&app);
    Ok(SettingsView {
        preset: settings.preset,
        protocol: settings.protocol,
        base_url: settings.base_url,
        model: settings.model,
        system_prompt: settings.system_prompt,
        theme: settings.theme,
        max_tokens: settings.max_tokens,
        font_size: settings.font_size,
        remember_position: settings.remember_position,
        default_zoom: settings.default_zoom,
        page_mode: settings.page_mode,
        spread_offset: settings.spread_offset,
        page_limit: settings.page_limit,
        request_timeout: settings.request_timeout,
        search_provider: settings.search_provider.clone(),
        has_key: has_key(&app),
        has_search_key: search_key_for(&app, &settings.search_provider).is_some(),
    })
}

/// Stores the configuration and, when supplied, the key.
#[tauri::command]
pub(crate) fn save_settings(app: tauri::AppHandle, settings: SettingsInput) -> Result<(), String> {
    if !(settings.base_url.starts_with("http://") || settings.base_url.starts_with("https://")) {
        return Err("接口地址必须以 http:// 或 https:// 开头".to_owned());
    }
    if settings.model.trim().is_empty() {
        return Err("模型名不能为空".to_owned());
    }

    if let Some(key) = settings.api_key.as_deref().map(str::trim) {
        if !key.is_empty() {
            write_key(&app, key)?;
        }
    }
    let provider = settings
        .search_provider
        .clone()
        .unwrap_or_else(default_search_provider);
    if let Some(key) = settings.search_api_key.as_deref().map(str::trim) {
        if !key.is_empty() {
            write_search_key(&app, &provider, key)?;
        }
    }

    let stored_window = read_settings(&app).window;
    let stored = Settings {
        preset: settings.preset,
        protocol: settings.protocol,
        base_url: settings.base_url.trim_end_matches('/').to_owned(),
        model: settings.model,
        system_prompt: settings
            .system_prompt
            .filter(|prompt| !prompt.trim().is_empty())
            .unwrap_or_else(default_prompt),
        theme: settings.theme.unwrap_or_else(default_theme),
        max_tokens: settings.max_tokens,
        font_size: settings.font_size.unwrap_or_else(default_font_size),
        remember_position: settings.remember_position.unwrap_or(true),
        default_zoom: settings.default_zoom.unwrap_or_else(default_zoom),
        page_mode: settings.page_mode.unwrap_or_else(default_mode),
        spread_offset: settings.spread_offset.unwrap_or(false),
        page_limit: settings.page_limit.unwrap_or(0),
        request_timeout: settings.request_timeout.unwrap_or(0),
        search_provider: settings
            .search_provider
            .unwrap_or_else(default_search_provider),
        window: stored_window,
    };

    let path = settings_path(&app)?;
    let json = serde_json::to_string_pretty(&stored)
        .map_err(|error| format!("无法序列化设置：{error}"))?;
    std::fs::write(path, json).map_err(|error| format!("无法写入设置：{error}"))?;
    Ok(())
}

/// Searches the web through the configured provider.
#[tauri::command]
pub(crate) async fn web_search(
    app: tauri::AppHandle,
    query: String,
) -> Result<Vec<pdfscope_ai::search::WebResult>, String> {
    let settings = read_settings(&app);
    let provider = pdfscope_ai::search::Provider::from_id(&settings.search_provider);
    let key = search_key_for(&app, provider.id()).ok_or_else(|| {
        if provider.reuses_model_key() {
            "还没有保存 API Key（DeepSeek 搜索复用模型 Key）".to_owned()
        } else {
            format!("还没有保存 {} 的搜索 Key", provider.label())
        }
    })?;
    pdfscope_ai::search::search(provider, &pdfscope_ai::Secret::new(key), &query)
        .await
        .map_err(|error| error.to_string())
}

/// Sends one tiny request so the reader can find out whether the settings work before saving them.
#[tauri::command]
pub(crate) async fn test_settings(
    app: tauri::AppHandle,
    settings: SettingsInput,
) -> Result<String, String> {
    let key = match settings
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    {
        Some(key) => pdfscope_ai::Secret::new(key),
        None => {
            let stored = read_key(&app).ok_or_else(|| "还没有保存 API Key".to_owned())?;
            pdfscope_ai::Secret::new(stored)
        }
    };

    let protocol = if settings.protocol == "anthropic" {
        Protocol::Anthropic
    } else {
        Protocol::OpenAi
    };

    let config = ProviderConfig::new(
        settings.preset,
        protocol,
        settings.base_url,
        settings.model,
        key,
    )
    .map_err(|error| error.to_string())?;

    let request = ChatRequest {
        system: None,
        messages: vec![Message::user("只回复 ok")],
        max_tokens: Some(16),
        temperature: None,
    };

    let mut events = pdfscope_ai::stream(&config, &request)
        .await
        .map_err(|error| error.to_string())?;

    while let Some(event) = events.next().await {
        match event {
            Ok(StreamEvent::Done) => break,
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
    }

    Ok(format!("连接正常 · {}", config.model()))
}

/// Reads the configuration and the key, for the request layer.
pub(crate) fn provider(app: &tauri::AppHandle) -> Result<(Settings, pdfscope_ai::Secret), String> {
    let settings = read_settings(app);
    let key = read_key(app).ok_or_else(|| "还没有保存 API Key".to_owned())?;
    Ok((settings, pdfscope_ai::Secret::new(key)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_files_get_defaults_for_new_fields() {
        let json = r#"{"preset":"MiMo","protocol":"openai","base_url":"https://x","model":"m"}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.font_size, 14);
        assert!(settings.remember_position);
        assert_eq!(settings.max_tokens, None);
    }

    #[test]
    fn window_geometry_survives_a_round_trip() {
        let settings = Settings {
            window: Some(WindowState {
                width: 1280,
                height: 800,
                x: 40,
                y: 60,
                maximized: true,
            }),
            ..Settings::default()
        };

        let json = serde_json::to_string(&settings).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(
            back.window,
            Some(WindowState {
                width: 1280,
                height: 800,
                x: 40,
                y: 60,
                maximized: true,
            })
        );
    }

    #[test]
    fn preferences_survive_a_round_trip() {
        let settings = Settings {
            font_size: 18,
            remember_position: false,
            ..Settings::default()
        };

        let json = serde_json::to_string(&settings).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(back.font_size, 18);
        assert!(!back.remember_position);
    }
}
