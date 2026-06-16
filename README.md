kuroya is a native code editor written in rust.

no electron. no webview. no javascript ui. the app is built with egui/eframe,
wgpu, tokio, ropey, tree-sitter, syntect, portable-pty, notify, and git2.

this repo is for people who want a hackable native editor with real editor
internals: rope-backed buffers, virtualized rendering, lsp, git, terminal,
session restore, crash recovery, and a command-driven runtime.

## status

kuroya is usable enough to build and test, but it is still early software.
there are a lot of implemented systems, and there is still a lot of polish left.

the current work is mostly around:

- editor correctness and responsiveness
- lsp edge cases
- terminal behavior
- git/source-control workflows
- workspace/session recovery
- vim/search/operator parity
- ui cleanup
- performance audits

extension marketplaces and vscode-style extension-host work are not planned.
local plugins, themes, syntax support, and commands are the intended direction.

## what works

- native desktop app with egui/eframe and wgpu
- rope-backed text buffers
- virtualized editor rendering
- tabs and split panes
- sidebar file explorer
- command palette
- quick open
- find/replace
- project search
- minimap
- diagnostics panel
- inline diagnostics and editor feedback
- lsp navigation, completion, hover, code actions, rename, symbols, folding,
  semantic tokens, inlay hints, and code lenses
- git status, diffs, hunks, history, stash, blame, branches, and commits
- integrated pty terminal
- session restore
- local history
- crash recovery
- guarded save/reload lifecycle
- settings, themes, keybindings, and optional vim-style keybindings
- large-file protections

## repo layout

```text
crates/
  kuroya-app/    desktop app, ui, and runtime integration
  kuroya-core/   buffers, search, settings, git, lsp, tasks, plugins, core logic
assets/          project assets
```

## requirements

- rust stable
- cargo
- a system supported by egui/wgpu

linux users may need the usual native windowing and gpu packages for winit/wgpu,
including x11/wayland libraries, `libxkbcommon`, and working vulkan/opengl
drivers.

## run it

```powershell
cargo run -p kuroya-app
```

## build it

```powershell
cargo build -p kuroya-app --release
```

the binary is `kuroya` on unix-like systems and `kuroya.exe` on windows.

## build the windows installer

install Inno Setup 6, then run:

```powershell
.\installer\build-installer.ps1
```

the setup exe is written to `dist\Kuroya-Setup-<version>.exe`. if `ISCC.exe`
is not on `PATH`, pass `-InnoCompilerPath` or set `INNO_SETUP_COMPILER`.

tagged GitHub builds publish the same setup exe as a release asset, which is
what the in-app update checker expects. release tags must match the app version,
for example `v0.1.0`.

## test it

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

for lockfile-based local validation:

```powershell
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

## hacking on kuroya

keep changes small and boring. this is editor code, so small regressions can
turn into very annoying daily-use bugs.

good rules:

- keep expensive work off the ui thread
- follow the existing command/runtime patterns
- keep core logic in `kuroya-core` when it does not need app state
- keep ui/runtime glue in `kuroya-app`
- avoid unrelated rewrites
- add regression tests for behavior changes
- preserve raw paths/data internally and sanitize only display text
- do not add telemetry, analytics, electron, webviews, or a vscode-style
  extension host

## good first areas

- focused bug fixes with tests
- smaller lsp edge cases
- source-control workflow fixes
- terminal rendering/lifecycle fixes
- search and quick-open behavior
- settings parsing and validation
- keyboard navigation polish
- test cleanup and module splitting
- documentation for existing behavior

## contributing

pull requests are welcome. a good pr should explain:

- what changed
- why it changed
- what behavior is covered by tests
- what was manually checked

before sending a pr, run the validation commands above. if a command cannot run
on your machine, say which one and why.

## roadmap

- [x] native rust desktop shell with egui/eframe and wgpu
- [x] rope-backed editor buffers
- [x] tabs, split panes, explorer, command palette, and quick open
- [x] find/replace, project search, minimap, diagnostics, and status ui
- [x] lsp basics: completion, hover, navigation, code actions, rename, symbols,
  folding, semantic tokens, inlay hints, and code lenses
- [x] git basics: status, diffs, hunks, history, stash, blame, branches, and
  commit flows
- [x] integrated pty terminal
- [x] session restore, local history, crash recovery, and guarded save/reload
- [x] settings, themes, keybindings, and optional vim-style keybindings
- [x] large-file protections and performance-oriented caches
- [ ] tighten editor correctness around selections, undo/redo, folding,
  snippets, indentation, ime, unicode, and large files
- [ ] keep improving render performance, minimap behavior, cache invalidation,
  and typing latency on large workspaces
- [ ] expand tree-sitter coverage, including language configs, injections,
  semantic highlighting, syntax-aware indentation, and ast-aware
  selection/folding
- [ ] harden lsp request lifecycles, stale response handling, completion
  ranking, workspace edits, and multi-server edge cases
- [ ] improve source-control workflows around stale repository state, conflicts,
  commits, hunks, branches, stash, blame, and history
- [ ] polish terminal rendering, search, persistence, process lifecycle, shell
  integration, and platform-specific behavior
- [ ] improve workspace flows like project indexing, quick open, project search,
  tasks, session restore, crash recovery, and local history
- [ ] continue vim mode parity, especially search flows, operators, text
  objects, registers, repeat behavior, visual mode edge cases, and command
  feedback
- [ ] add more end-to-end smoke coverage and platform validation for windows,
  linux, and macos

## license

kuroya is licensed under apache-2.0.
