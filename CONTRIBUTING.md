# Contributing to CrewMate Core

This repo is the backend shared by the CrewMate aircraft apps. Read the [README](README.md) first: it describes what belongs here, and the commands and events every change must keep.

**What goes here:** logic that is the same in every aircraft. Aircraft data stays in each aircraft repo, along with everything that makes that app its own product: windows, flows, checklists, grammar, sounds, settings UI, identifiers and keys. If a change only makes sense for one aircraft, it belongs in that app, through `Config::setup` or its own commands.

## Setup

- Windows 10 or 11.
- Rust stable, 1.80 or newer (`rustup`).
- **The MSFS SDK**. Install it from the SDK download in Microsoft Flight Simulator's developer mode, and set `MSFS_SDK` to its folder (for example `C:\MSFS 2024 SDK`); without the variable, the build looks in `C:\MSFS SDK`. The `msfs` crate links SimConnect from it.
- **LLVM** (`winget install LLVM.LLVM`), with `LIBCLANG_PATH` set to its `bin` folder (for example `C:\Program Files\LLVM\bin`). The `msfs` crate generates its bindings with bindgen.
- An aircraft app cloned next to this repo (for example `..\CrewMateA350`), to test changes in a real app. Follow that app's Contributing guide to set it up.

```powershell
git clone https://github.com/CrewMate-Flight-Sim/CrewMate-Core.git
cd CrewMate-Core
cargo check
```

## Making a change

1. Branch from `main`. `main` is protected, so every change goes through a PR, the maintainers' too.
2. Make the change in `crates/crewmate-core`. Keep aircraft names, LVars and values out of the code: anything that differs between aircraft is a `Config` field or lives in the app.
3. Run the checks, all of them clean:
   ```powershell
   cargo fmt
   cargo clippy --all-targets --all-features -- -D warnings
   cargo check
   ```
4. Test it in an aircraft app. In the app's `src-tauri` folder, create `.cargo/config.toml` (the apps gitignore it):
   ```toml
   [patch."https://github.com/CrewMate-Flight-Sim/CrewMate-Core"]
   crewmate-core = { path = "../../CrewMate-Core/crates/crewmate-core" }
   ```
   Then run `npm run tauri dev` in the app. It builds against your checkout instead of the pinned tag. Exercise what you changed and read the app log. While the file exists, the app's `Cargo.lock` points at your local path, so don't commit it in that state. Delete the file afterwards; the next build goes back to the pinned version. A build made this way is for testing only.
5. Open a PR. Describe what changed and how you tested it, and in which app. CI checks the formatting; it can't compile without the MSFS SDK, so the PR checklist asks you to confirm the local checks passed.

If your change also needs code in an aircraft app (for example, the frontend reading a new event), open that app's PR too and write "needs CrewMate-Core vX.Y.Z" in it. It gets merged once the core version is tagged.

### Code style

- Comments are one line and say why, not what.
- Every log line starts with a `[Module]` prefix (`[Speech]`, `[Audio]`, `[SimConnect]`, …), and existing log lines keep their wording.
- No module-path stutter: `audio::player`, not `audio::audio_player`.

### Compatibility rules

Apps pin an exact version and can be several versions behind, so:

- **Patch** (`1.0.x`): a fix that changes no name, payload or `Config` field.
- **Minor** (`1.x.0`): something new that existing apps don't have to use. A new command, a new event, a new field in a payload, or a new `Config` field that has a sensible default.
- **Major** (`x.0.0`): anything an app must change for. A renamed or removed command or event, a changed payload, a changed `Config`, or a change to how `builder`, `handler!` or `install_panic_hook` are used. Write an "Upgrading" note in the CHANGELOG for it.

## Releasing (repo owners)

Releases are made by the repo owners. Contributors stop at a merged PR.

1. Merge the changes to `main`.
2. Add a section for the new version at the top of `CHANGELOG.md`. Write one line per change, from an app developer's point of view. Set `version` in `crates/crewmate-core/Cargo.toml` to match, and merge both through a PR.
3. Tag and push:
   ```powershell
   git switch main
   git pull
   git tag v1.1.0
   git push origin v1.1.0
   ```
4. **Never delete, move or re-push a tag.** Apps pin tags, and a moved tag breaks their builds. If a release is wrong, fix it in a new version.
5. Pin the new tag in CrewMate A350 first, and live-test it there. Each other app moves to it in its own release, when its maintainer decides; the app's Contributing guide has the steps.

## License

By contributing you agree that your contributions are licensed under GPL-3.0.
