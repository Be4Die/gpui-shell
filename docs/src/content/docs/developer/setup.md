---
title: Development setup
description: Build, run, test and profile GPUi Shell from source.
---

To run the shell, you need a Wayland session on Hyprland, Niri or Mango.
Building works anywhere.

## With Nix (recommended)

```bash
git clone https://github.com/andre-brandao/gpui-shell
cd gpui-shell
nix develop        # or `direnv allow`
cargo run
```

The dev shell provides the same toolchain (nightly, from
`rust-toolchain.toml`) and system libraries that CI uses.

## Without Nix

Install a nightly Rust toolchain via rustup (`rust-toolchain.toml` selects it
automatically), plus the development headers for:

`wayland`, `libxkbcommon`, `vulkan-loader`, `fontconfig`, `freetype`,
`openssl`, `libpulseaudio`, `pipewire`, `systemd` (libudev), `libxcb`, and
`clang` (for bindgen).

[`nix/shell.nix`](https://github.com/andre-brandao/gpui-shell/blob/main/nix/shell.nix)
has the authoritative list.

## Useful commands

```bash
cargo run                         # run the shell
cargo run -- --input ";s"         # open the launcher on a view (here: services)
RUST_LOG=debug cargo run          # verbose logging
cargo run -p story                # component gallery
cargo run -- --validate           # check config.toml / theme.toml and exit
scripts/flamegraph.sh --launch    # CPU profile
scripts/heaptrack.sh              # heap profile
```

A running shell owns the IPC socket, so a second `cargo run` only toggles
the launcher of the first one. Stop your installed shell before running a
dev build.

## Checks

CI runs these on every pull request:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
nix build
```

## Conventions

- **Timers:** use `cx.background_executor().timer(duration).await` inside
  GPUI tasks, never `tokio::time::sleep`.
- **Theme:** read colors and sizes from `cx.theme()`. Use `px()` for spacing
  and `theme.font_sizes.*` for text.
- **Icons:** add an SVG to `crates/assets/icons/` (from
  [Lucide](https://lucide.dev)). The asset bundle rebuilds automatically.
- **Logging:** use `tracing` macros (`debug!`, `warn!`, ...), not `println!`.
- **Config:** the shell never writes `config.toml`. Anything it persists goes
  to `state.toml`.
- **UI layering:** primitives go in `ui::components`, composite shell chrome
  in `ui::patterns`. State, keybindings and config stay in `app`. Every
  component or pattern gets a story in `crates/story`.

## The docs site

This site lives in `docs/` (Astro + Starlight):

```bash
cd docs
bun install
bun run dev
```

It deploys to GitHub Pages on every push to `main` that touches `docs/`.
