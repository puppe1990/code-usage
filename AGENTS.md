# Agent notes

macOS menu bar app (Tauri v2 + Rust) showing usage for Command Code, Grok and OpenCode. The panel
is plain TypeScript (Vite), no framework. `README.md` has the product story; this file is the
working rules.

## Commands (from the repo root)

| What                  | Command                                                                              |
| --------------------- | ------------------------------------------------------------------------------------ |
| Frontend tests        | `npm test`                                                                           |
| Frontend types        | `npx tsc --noEmit`                                                                   |
| Frontend build        | `npm run build`                                                                      |
| Rust tests            | `cargo test --release --manifest-path src-tauri/Cargo.toml`                          |
| Rust lint / format    | `cargo clippy --all-targets -- -D warnings` / `cargo fmt --all`                      |
| Smoke on this machine | `cargo test --release --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture` |
| Regenerate tray PNGs  | `node scripts/generate-icons.mjs`                                                    |
| Run the app           | `npm run tauri dev`                                                                  |

The pre-commit hook and CI run these per changed path; run them yourself before claiming done.

## Layout

- `src-tauri/src/usage/` — pure parsers: one module per CLI (`commandcode.rs`, `grok.rs`,
  `opencode.rs`), plan limits (`commandcode_api.rs`, `opencode_go.rs`), time windows (`window.rs`)
  and the tray title (`tray_title.rs`). `snapshot` in `usage/mod.rs` is the only entry point.
- `src-tauri/src/{tray,refresh,commands}.rs` — menu bar item, 60s loop, IPC commands.
- `src/` — panel: `render.ts` returns the HTML string, `main.ts` wires events, `format.ts` formats
  numbers. Tests sit next to the code (`*.test.ts`).
- `src-tauri/icons/*.svg` — the only source of the harness marks. `scripts/svg-mark.mjs` rasterizes
  them into the committed tray PNGs; never hand-edit a PNG, edit the SVG and regenerate.

## Rules

- Keep parsers pure and testable. Read paths through the `*_path()` helpers so tests can point them
  elsewhere with the `CODE_USAGE_*` environment variables.
- Frontend: no framework and no new runtime dependency without need. Render functions return strings
  and are covered by `render.test.ts`.
- Add a test for new behavior and a regression test for every bugfix. Prefer the fixtures in
  `src-tauri/tests/fixtures/` over live data.
- Use the default formatters (`prettier`, `cargo fmt`) instead of discussing style.
- Icon output is deterministic: `git diff src-tauri/icons` must be empty after regenerating.

## Caveats

- macOS-only code: the screen/scale handling in `tray.rs` and the autostart LaunchAgent.
- Plan limits come from undocumented endpoints behind a 5-minute cache; never assume they are fresh.
  The Command Code API is `alpha`: parse defensively and keep the last known value on failure.
