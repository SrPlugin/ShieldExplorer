# ShieldExplorer

A file explorer for Linux written in Rust with [GPUI](https://www.gpui.rs/), the GPU-accelerated
UI framework behind the [Zed](https://zed.dev) editor. It is built for **developers and technical
users** who want to move fast **without touching the mouse**: vim-style keybindings, a terminal-like
look, and the information that matters in plain sight (permissions, exact size, modification date).

> **Vibe-coded project.** I'm building this with the help of AI, for fun, to have a file explorer
> made to my own taste. A human decides what gets built and tests it; the code is written by an AI.
> Keep that in mind if you read the code or plan to use it: this is a personal project that is still
> evolving, not a finished product.

![ShieldExplorer in dark theme](docs/images/screenshot-dark.png)

## Status

Early version. It is already good for **browsing folders**. It does **not** yet open files, or
copy, move, delete or rename them (see the [roadmap](#roadmap)).

The logic (`shieldexplorer-core`) has automated tests. The UI is tested by hand: keys, clicks and
path editing have no automated tests yet.

## What it does today

- **Detailed listing** in the style of `ls -l`: permissions (`drwxr-xr-x`), size, modification date
  and name. Folders come first and end in `/`; symbolic links end in `@`.
- **Relative line numbers**, like vim's `relativenumber`, ready for counts such as `5j` later. The
  active row shows its absolute position.
- **Sort by column** (name, size, date), ascending or descending, with natural ordering (`f2`
  before `f10`) and case-insensitive names.
- **Virtualized list**: only the visible rows are drawn, so a huge folder does not slow the window.
- **Background loading**: the window never freezes while reading the disk. If you navigate quickly,
  stale loads are discarded.
- **Path bar** with clickable segments and an edit mode (accepts `~`, `.`, `..` and relative paths).
- **Back and forward history.**
- **Errors that don't break anything**: a folder without permission or a missing one shows
  `error[EACCES]` or `error[ENOENT]`, and you stay where you were.
- **Light and dark theme** that follows the system and switches on its own.
- **Status bar** with the mode, the active file's permissions and exact byte size, its position,
  and the folder total.

## Keybindings

### List

| Key                     | Action                                                        |
| ----------------------- | ------------------------------------------------------------- |
| `j` / `↓`               | Move down                                                     |
| `k` / `↑`               | Move up                                                       |
| `l` / `→` / `Enter`     | Open the selected folder                                      |
| `h` / `←` / `Backspace` | Go to the parent folder (the cursor lands on the one you left) |
| `gg` / `Home`           | First row                                                     |
| `G` / `End`             | Last row                                                      |
| `i`                     | Edit the path                                                 |
| `Alt+←` / `Alt+→`       | Back / forward in history                                     |
| `Esc`                   | Dismiss an error message                                      |
| `Ctrl+Q`                | Quit                                                          |

### While editing the path

`Enter` goes to that path and `Esc` cancels. Move with `←` `→` `Home` `End`, delete with
`Backspace` and `Delete`, and paste with `Ctrl+V`.

### Mouse (optional)

Click selects, double-click opens a folder, clicking a column header sorts, clicking a path segment
jumps to that level, and the wheel scrolls the list.

## Safety and robustness

- `unsafe_code` is **forbidden** in the project's own code (dependencies do use it).
- It does not run shell commands or open files.
- File names are treated as what they are on Linux: bytes that need not be valid UTF-8.
- No `unwrap`, `expect` or `panic!` in the code; errors are typed.
- Symbolic links are shown as links and broken ones are flagged. There is no recursive traversal,
  so there is no risk of loops.

## Building and running

You need:

- Rust with edition 2024 support (1.85 or later). Tested with 1.92.
- Linux with Wayland or X11 and a graphics driver with Vulkan.
- Development headers for xkbcommon, Wayland, xcb and fontconfig. On the development machine
  (Ubuntu) these are installed: `libxkbcommon-dev`, `libxkbcommon-x11-dev`, `libwayland-dev`,
  `libxcb1-dev` and `libfontconfig-dev`. If the build fails on a missing library, start there.

```sh
# Open in the current folder, or in the one you pass
cargo run -p shieldexplorer-app
cargo run -p shieldexplorer-app -- ~/projects

# Optimized binary at target/release/shieldexplorer
cargo build --release
```

If you pass the path of a file, its folder opens with that file selected.

### Development

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The first build is slow because GPUI pulls in many dependencies.

## Layout

```
ShieldExplorer/
├── crates/
│   ├── shieldexplorer-core/   UI-free logic: read folders, sorting, history, paths, formatting
│   └── shieldexplorer-app/    GPUI window: state, list, path bar, status bar, theme
├── assets/fonts/              JetBrains Mono (OFL license)
└── docs/superpowers/specs/    design document and visual mockup
```

Dependencies point one way: `shieldexplorer-app` uses `shieldexplorer-core` and never the reverse,
so the logic can be tested without opening a window. The full design is in
[`docs/superpowers/specs/2026-11-16-explorer-nucleo-design.md`](docs/superpowers/specs/2026-11-16-explorer-nucleo-design.md)
(written in Spanish).

`gpui` is pinned to `=0.2.2` because its API is 0.x and changes between releases. `Cargo.lock`
starts from the one published with that version: with a recent `libc`, `xattr 0.2.3` does not
compile.

## Roadmap

No dates and no promises, ordered by what best fits the keyboard-first, productivity-minded focus:

1. **Full vim mode:** counts (`5j`), incremental search with `/` and `n`/`N`, a `:` command line,
   marks, `Ctrl+D`/`Ctrl+U`, `H`/`M`/`L`, toggling hidden files.
2. **Keyboard file operations:** copy (`yy`), cut, paste (`p`), delete to trash, rename, create
   files and folders, visual selection (`v`).
3. **Hooks into the dev environment:** open with the default program or `$EDITOR`, copy the path to
   the clipboard, open a terminal in the folder.
4. **Fast fuzzy search** in the spirit of fzf that respects `.gitignore`, plus folder bookmarks.
5. **More context for devs:** git status in the listing and a preview of text files.
6. **Configuration:** custom keybindings and colors.
7. Side tree and tabs.

## Credits

- [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui), by Zed Industries.
- [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), under the SIL OFL 1.1 license.

## License

To be decided.
