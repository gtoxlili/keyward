//! Tauri backend for the Keyward Executor desktop app.
//!
//! Every command drives the *real* executor core from the `keyward` crate — there is no
//! reimplementation here. The UI starts the executor with a config, and structured
//! [`ExecutorEvent`]s are streamed back over an IPC [`Channel`]. Provider credentials
//! live in the OS keychain and never cross to the frontend.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::async_runtime::{JoinHandle, Mutex};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use keyward::executor::{self, ExecutorConfig, ExecutorEvent};
use keyward::keyward_proto::{Budget, Policy, Rate};
use keyward::{identity, secret, wire};

/// The currently running executor task, so a new start (or stop) can cancel it.
#[derive(Default)]
struct Runner(Mutex<Option<JoinHandle<()>>>);

/// Config the UI submits to start the executor.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartConfig {
    orch_url: String,
    pairing_token: String,
    providers: Vec<String>,
    budget_usd: Option<f64>,
    rpm: Option<u32>,
    /// Out-of-band root fingerprint to pin (refuse anything else, §3/§9).
    expected_root_fp: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityInfo {
    pubkey: String,
    fingerprint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KeyStatus {
    provider: String,
    present: bool,
}

/// This Executor's long-term identity (created + persisted on first call). The
/// pubkey is what an Orchestrator allow-lists.
#[tauri::command]
async fn get_identity() -> Result<IdentityInfo, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let vk = identity::load_or_create_identity().verifying_key().to_bytes();
        IdentityInfo {
            pubkey: wire::hex(&vk),
            fingerprint: wire::fingerprint(&vk),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Store a provider credential in the OS keychain.
#[tauri::command]
async fn set_key(provider: String, key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || secret::store_key(&provider, &key))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Remove a stored provider credential.
#[tauri::command]
async fn delete_key(provider: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || secret::delete_key(&provider))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Which of `providers` currently have a resolvable credential (keychain or env).
#[tauri::command]
async fn key_status(providers: Vec<String>) -> Result<Vec<KeyStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        providers
            .into_iter()
            .map(|p| KeyStatus {
                present: secret::key_present(&p),
                provider: p,
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Start the executor: build a policy + identity, dial the Orchestrator, and stream
/// status events back over `on_event`. Cancels any previously running executor.
#[tauri::command]
async fn start_executor(
    runner: State<'_, Runner>,
    config: StartConfig,
    on_event: Channel<ExecutorEvent>,
) -> Result<(), String> {
    // Cancel any in-flight executor first.
    if let Some(handle) = runner.0.lock().await.take() {
        handle.abort();
    }

    // Out-of-band pin: the executor reads this when verifying the orchestrator root.
    match &config.expected_root_fp {
        Some(fp) if !fp.trim().is_empty() => {
            // SAFETY: set once at startup before the executor task spins up.
            unsafe { std::env::set_var("KEYWARD_EXPECT_ROOT_FP", fp.trim()) };
        }
        _ => unsafe { std::env::remove_var("KEYWARD_EXPECT_ROOT_FP") },
    }

    let policy = Policy {
        providers: Some(config.providers.clone()),
        budget: config.budget_usd.map(|limit_usd| Budget {
            limit_usd,
            window: "month".into(),
            spent_usd: 0.0,
        }),
        rate: config.rpm.map(|rpm| Rate {
            rpm: Some(rpm),
            tpm: None,
        }),
        ..Default::default()
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ExecutorEvent>();
    // Forward executor events to the frontend channel until the executor task ends
    // (which drops `tx` and closes this loop).
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let _ = on_event.send(ev);
        }
    });

    let exec_cfg = ExecutorConfig {
        name: "keyward-desktop".into(),
        providers: config.providers,
        policy,
        keys: secret::KeySource::Keychain,
        identity: identity::load_or_create_identity(),
        pinned: Arc::new(Mutex::new(None)),
        events: Some(tx),
    };
    let url = config.orch_url;
    let token = config.pairing_token;

    let handle = tauri::async_runtime::spawn(async move {
        let _ = executor::run(&url, &token, exec_cfg).await;
    });
    *runner.0.lock().await = Some(handle);
    Ok(())
}

/// Stop the running executor (if any).
#[tauri::command]
async fn stop_executor(runner: State<'_, Runner>) -> Result<(), String> {
    if let Some(handle) = runner.0.lock().await.take() {
        handle.abort();
    }
    Ok(())
}

fn settings_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

/// Load the UI's persisted settings blob (language, theme, last orchestrator, policy
/// defaults…). The frontend owns the shape. Returns `null` if none saved yet.
#[tauri::command]
fn load_settings(app: AppHandle) -> serde_json::Value {
    settings_file(&app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Persist the UI settings blob.
#[tauri::command]
fn save_settings(app: AppHandle, settings: serde_json::Value) -> Result<(), String> {
    let path = settings_file(&app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Runner::default())
        .invoke_handler(tauri::generate_handler![
            get_identity,
            set_key,
            delete_key,
            key_status,
            start_executor,
            stop_executor,
            load_settings,
            save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Keyward Executor");
}
