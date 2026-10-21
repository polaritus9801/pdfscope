//! Where the model configuration lives.
//!
//! The non-secret half is a small JSON file in the app config directory; the key goes to the
//! operating system's keychain and is never written next to the settings, never returned to the
//! webview, and never logged.

use std::path::PathBuf;

use futures::StreamExt;
use pdfscope_ai::{ChatRequest, Message, Protocol, ProviderConfig, StreamEvent};
use serde::{Deserialize, Serialize};
use tauri::Manager;

const KEYCHAIN_SERVICE: &str = "app.pdfscope.desktop";
const KEYCHAIN_ACCOUNT: &str = "api-key";

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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preset: "MiMo".to_owned(),
            protocol: "openai".to_owned(),
            base_url: "https://api.xiaomimimo.com/v1".to_owned(),
            model: "mimo-v2.6-flash".to_owned(),
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
    /// A new key, when the user typed one. `None` keeps the stored key.
    pub(crate) api_key: Option<String>,
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
    /// Whether a key is stored. The key itself never crosses this boundary.
    pub(crate) has_key: bool,
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

fn has_key() -> bool {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .is_some_and(|key| !key.trim().is_empty())
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
        has_key: has_key(),
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
            let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
                .map_err(|error| format!("无法访问钥匙串：{error}"))?;
            entry
                .set_password(key)
                .map_err(|error| format!("无法写入钥匙串：{error}"))?;
        }
    }

    let stored = Settings {
        preset: settings.preset,
        protocol: settings.protocol,
        base_url: settings.base_url.trim_end_matches('/').to_owned(),
        model: settings.model,
    };

    let path = settings_path(&app)?;
    let json = serde_json::to_string_pretty(&stored)
        .map_err(|error| format!("无法序列化设置：{error}"))?;
    std::fs::write(path, json).map_err(|error| format!("无法写入设置：{error}"))?;
    Ok(())
}

/// Sends one tiny request so the reader can find out whether the settings work before saving them.
#[tauri::command]
pub(crate) async fn test_settings(settings: SettingsInput) -> Result<String, String> {
    let key = match settings
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    {
        Some(key) => pdfscope_ai::Secret::new(key),
        None => {
            let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
                .map_err(|error| format!("无法访问钥匙串：{error}"))?;
            let stored = entry
                .get_password()
                .map_err(|_| "还没有保存 API Key".to_owned())?;
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
        max_tokens: 16,
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
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|error| format!("无法访问钥匙串：{error}"))?;
    let key = entry
        .get_password()
        .map_err(|_| "还没有保存 API Key".to_owned())?;
    Ok((settings, pdfscope_ai::Secret::new(key)))
}
