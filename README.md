# CrewMate Core

The shared backend of the CrewMate aircraft apps ([A350](https://github.com/CrewMate-Flight-Sim/CrewMateA350), [A310](https://github.com/CrewMate-Flight-Sim/CrewMateA310), [MD-11](https://github.com/CrewMate-Flight-Sim/CrewMateMD11)). A fix made here reaches every aircraft, and a new aircraft starts from it.

This is a build-time library, not a product. Each aircraft app is a separate product with its own installer, version, release schedule and updater. Each one pins an exact version of this repo and moves to a newer one whenever its own release is ready. Users never see this repo; bugs are reported in the aircraft repos.

What's here today:

- `crates/crewmate-core`: the Rust side of the Tauri app.
- A TypeScript package for the shared frontend code will join this repo later, at the repository root.

## What core owns

Everything that is the same in every aircraft:

- **The app shell** (`builder`): the Tauri plugins, logging, the close confirmation, sidecar shutdown and the panic hook.
- **Audio**: sound packs, playback queue, input and output devices.
- **Speech bridge**: runs the `copilot_speech` sidecar from [CrewMate-Voice](https://github.com/CrewMate-Flight-Sim/CrewMate-Voice), restarts it, and forwards what it recognises.
- **Input**: push-to-talk and the mic toggle, on keyboard and joystick.
- **SimConnect**: simvar and LVar reads and writes, telemetry streaming, the aircraft title, and cockpit detection.
- **App data**: the log file, and the app data and log folders.
- **Modal windows**: the `create_modal_window` helper, always on top, and close.

Each app keeps whatever makes it its own product:

- `tauri.conf.json`: identifier, product name, updater key and endpoint.
- `capabilities/`.
- Its windows, with their sizes and pages.
- The `Config` values below.
- The whole frontend: flows, checklists, grammar, sounds and settings UI.

## Using it in an app

`src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri = { version = "2", features = [] }
crewmate-core = { git = "https://github.com/CrewMate-Flight-Sim/CrewMate-Core", tag = "v1.0.0" }
serde_json = "1"
# Unused in code: tauri-build only finds the permissions capabilities/ names in direct dependencies
tauri-plugin-dialog = "2"
tauri-plugin-log = "2"
tauri-plugin-opener = "2"

[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
tauri-plugin-updater = "2"
tauri-plugin-window-state = "2"
```

Two of these look unused but are required:
- **`serde_json`**: `tauri::generate_context!()` needs it in the app crate.
- **The plugin crates**: `tauri-build` resolves the permissions that `capabilities/*.json` names (`dialog:default`, `updater:default`, …) only from the app's direct dependencies.

`src-tauri/src/lib.rs`:

```rust
mod windows;

const CONFIG: crewmate_core::Config = crewmate_core::Config {
    app_name: "Crewmate INI A350",
    log_file_stem: "crewmateinia350",
    modal_windows: &["takeoff", "landing", "settings"],
    setup: None,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    crewmate_core::builder(CONFIG)
        .invoke_handler(crewmate_core::handler![
            windows::open_landing_window,
            windows::open_settings_window,
            windows::open_takeoff_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    crewmate_core::install_panic_hook();
    crewmatea350_lib::run()
}
```

The app's own windows call `crewmate_core::create_modal_window(&app_handle, label, url, title, width, height, resizable)`.

### `Config`

| Field | What it is |
|---|---|
| `app_name` | Shown in the `[App] <app_name> application loaded...` log line. |
| `log_file_stem` | The log file's name, `logs\<stem>.log` under the app data folder. Frozen once an app has shipped, because users send this file. |
| `modal_windows` | Labels of the app's modal windows. The window-state plugin doesn't restore them. |
| `setup` | Optional `fn(&mut tauri::App)` for the app's own Rust state or threads. Runs after core's setup. |

Tauri allows only one `setup` and one `invoke_handler` per app. Core uses the `setup`; an app adds its own through `Config::setup`. The app's own commands go into `handler![…]`, after core's.

## The contract with the frontend

The frontend of every app depends on these names. Renaming or removing one, or changing a payload, is a major version (see [CONTRIBUTING](CONTRIBUTING.md)).

**Commands** registered by `handler!`:

- **Windows**: `close_app`, `set_always_on_top`.
- **App data**: `get_log_file_path`, `open_app_data_folder`, `open_logs_folder`.
- **SimConnect**: `simvar_set`, `simvar_get`, `start_telemetry_stream`, `stop_telemetry_stream`, `get_aircraft_title`, `get_in_cockpit`.
- **Audio**: `play_sound`, `play_sound_sequence`, `is_audio_playing`, `get_sound_packs`, `get_available_input_devices`, `get_available_output_devices`, `set_output_device`, `set_input_device`.
- **Speech**: `get_speech_engine_error`, `set_confidence_threshold`, `get_speech_input_devices`.
- **Input**: `set_muted`, `set_voice_mode`, `set_mic_bindings`, `start_input_capture`, `cancel_input_capture`.

**Events** emitted to the frontend:

- **App**: `close-requested`.
- **SimConnect**: `telemetry_data`, `simconnect-aircraft-title`, `sim-in-flight`.
- **Speech**: `speech_recognized`, `speech_engine_status`, `speech_engine_error`.
- **Input**: `ptt_state`, `mic_toggle_pressed`, `input_captured`, `input_capture_cancelled`.

The sidecar is always called `copilot_speech`. Every log line keeps its `[Module]` prefix and wording, because users and maintainers search logs for them.

## Starting a new aircraft

1. Create the new app's repo from an existing app.
2. Choose its frozen values on day one. They can never change after the first release:
   - repo name;
   - `identifier` and `productName`;
   - a new updater key pair;
   - `log_file_stem`.
3. Copy `main.rs`, `lib.rs` and `windows.rs` from the example above, with the new values.
4. Pin the latest core tag.

Anything the new aircraft needs that the others don't belongs in the app, through `Config::setup` and its own commands. If it turns out to be generic, it belongs here instead.

## Building

Building needs the MSFS SDK and LLVM, because the `msfs` crate links SimConnect statically and generates its bindings with bindgen. See [CONTRIBUTING](CONTRIBUTING.md#setup).

## License

GPL-3.0, like the aircraft apps. See [LICENSE](LICENSE).
