use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

pub mod app_data;
pub mod audio;
pub mod bridges;
pub mod input;
pub mod simconnect;
pub mod windows;

use app_data::{setup_app_data_directories, LOGS_DIR_NAME};
use audio::commands::AudioPlayerState;
use audio::player::AudioPlayer;
use bridges::speech_bridge::{SpeechBridge, SpeechBridgeState, SPEECH_BRIDGE_STATE};
use simconnect::aircraft_title::start_aircraft_title_stream;
use simconnect::flight_state::start_flight_state_stream;
use simconnect::simvars::{spawn_simvar_worker, SimVarState};

pub use windows::create_modal_window;

pub type AppSetup = fn(&mut tauri::App) -> Result<(), Box<dyn std::error::Error>>;

/// What differs between aircraft apps; everything else in the shell is shared.
#[derive(Clone, Copy)]
pub struct Config {
    /// Shown in the "application loaded" log line, e.g. "Crewmate INI A350".
    pub app_name: &'static str,
    /// Base name of the log file, e.g. "crewmateinia350". Frozen per app: users send this file.
    pub log_file_stem: &'static str,
    /// Window labels the window-state plugin must not restore (the app's modal windows).
    pub modal_windows: &'static [&'static str],
    /// The app's own setup, run after core's.
    pub setup: Option<AppSetup>,
}

/// Kills the speech sidecar on a panic from any thread; call first thing in `main`.
pub fn install_panic_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        if let Some(speech) = SPEECH_BRIDGE_STATE.get() {
            speech.shutdown();
        }
        prev_hook(panic_info);
    }));
}

/// The app's Tauri builder with every plugin, state and background stream set up.
/// The app adds `.invoke_handler(crewmate_core::handler![…])` and runs it.
pub fn builder(config: Config) -> tauri::Builder<tauri::Wry> {
    #[cfg(debug_assertions)]
    std::env::set_var(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "--remote-debugging-port=9222",
    );
    let worker_tx = spawn_simvar_worker();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .with_denylist(config.modal_windows)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app
                .get_webview_window("main")
                .expect("no main window")
                .set_focus();
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_prevent_default::debug())
        .setup(move |app| {
            app.manage(config);

            // Initialize speech recognition sidecar
            let speech = Arc::new(SpeechBridge::new(app.handle().clone()));
            SPEECH_BRIDGE_STATE.set(speech.clone()).ok();
            app.manage(SpeechBridgeState {
                bridge: speech.clone(),
            });

            // DirectInput needs a top-level window of ours to read joysticks in the background
            let main_hwnd = app
                .get_webview_window("main")
                .and_then(|w| w.hwnd().ok())
                .map_or(0, |h| h.0 as isize);
            app.manage(input::start(app.handle().clone(), main_hwnd));

            // Initialize audio player
            let audio_player = AudioPlayer::new().expect("Failed to initialize audio player");
            app.manage(AudioPlayerState(std::sync::Mutex::new(audio_player)));

            // Initialize SimVar worker
            app.manage(SimVarState {
                tx: Mutex::new(worker_tx),
            });

            // Start aircraft title SimConnect stream
            start_aircraft_title_stream(app.handle().clone());
            start_flight_state_stream(app.handle().clone());

            // Initialize logging
            let logs_dir = match app.path().app_data_dir() {
                Ok(app_data_dir) => {
                    let logs_path = app_data_dir.join(LOGS_DIR_NAME);
                    if let Err(e) = std::fs::create_dir_all(&logs_path) {
                        eprintln!("[App] Failed to create logs directory: {}", e);
                        app_data_dir
                    } else {
                        logs_path
                    }
                }
                Err(e) => {
                    eprintln!("[App] Failed to get app data directory: {}", e);
                    std::path::PathBuf::from(".")
                }
            };

            let log_plugin = tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: logs_dir,
                        file_name: Some(config.log_file_stem.to_string()),
                    },
                ))
                .level(log::LevelFilter::Info)
                .build();

            app.handle()
                .plugin(log_plugin)
                .expect("Failed to initialize logging plugin");

            log::info!("[App] {} application loaded...", config.app_name);

            if let Err(e) = setup_app_data_directories(app.handle()) {
                log::error!("[App] Failed to setup app data directories: {}", e);
            }

            // Close request handling
            let should_close = Arc::new(AtomicBool::new(false));
            app.manage(should_close.clone());

            if let Some(window) = app.get_webview_window("main") {
                let window_for_closure = window.clone();
                let speech_for_close = SPEECH_BRIDGE_STATE.get().cloned();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if should_close.load(Ordering::SeqCst) {
                            return;
                        }
                        api.prevent_close();
                        let _ = window_for_closure.emit("close-requested", ());
                    }
                    if let tauri::WindowEvent::Destroyed = event {
                        if let Some(speech) = speech_for_close.as_ref() {
                            speech.shutdown();
                        }
                    }
                });
            }

            if let Some(app_setup) = config.setup {
                app_setup(app)?;
            }

            Ok(())
        })
}

/// Every core command plus the app's own, for `.invoke_handler(…)`:
/// `crewmate_core::handler![windows::open_settings_window, …]`.
#[macro_export]
macro_rules! handler {
    ($($app_commands:tt)*) => {
        ::tauri::generate_handler![
            $crate::windows::close_app,
            $crate::windows::set_always_on_top,
            $crate::app_data::get_log_file_path,
            $crate::app_data::open_app_data_folder,
            $crate::app_data::open_logs_folder,
            $crate::simconnect::simvars::simvar_set,
            $crate::simconnect::simvars::simvar_get,
            $crate::simconnect::simvars::start_telemetry_stream,
            $crate::simconnect::simvars::stop_telemetry_stream,
            $crate::audio::commands::play_sound,
            $crate::audio::commands::play_sound_sequence,
            $crate::audio::commands::is_audio_playing,
            $crate::audio::commands::get_sound_packs,
            $crate::audio::devices::get_available_input_devices,
            $crate::audio::devices::get_available_output_devices,
            $crate::audio::devices::set_output_device,
            $crate::audio::devices::set_input_device,
            $crate::simconnect::aircraft_title::get_aircraft_title,
            $crate::simconnect::flight_state::get_in_cockpit,
            $crate::bridges::speech_bridge::get_speech_engine_error,
            $crate::bridges::speech_bridge::set_confidence_threshold,
            $crate::bridges::speech_bridge::get_speech_input_devices,
            $crate::input::set_muted,
            $crate::input::set_voice_mode,
            $crate::input::set_mic_bindings,
            $crate::input::start_input_capture,
            $crate::input::cancel_input_capture,
            $($app_commands)*
        ]
    };
}
