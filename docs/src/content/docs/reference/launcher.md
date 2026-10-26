---
title: Launcher
description: Launcher configuration reference.
---

The `[launcher]` section controls the command launcher window dimensions and placement.

## Options

| Option          | Type    | Default | Description                              |
| --------------- | ------- | ------- | ---------------------------------------- |
| `width`         | `float` | `600.0` | Launcher window width in pixels.         |
| `height`        | `float` | `450.0` | Launcher window height in pixels.        |
| `margin_top`    | `float` | `100.0` | Top margin for layer-shell placement.    |
| `margin_right`  | `float` | `0.0`   | Right margin for layer-shell placement.  |
| `margin_bottom` | `float` | `0.0`   | Bottom margin for layer-shell placement. |
| `margin_left`   | `float` | `0.0`   | Left margin for layer-shell placement.   |

## Example

```toml
[launcher]
width = 700.0
height = 500.0
margin_top = 150.0
```

## Launcher Modules

The launcher is made of modules, each opened by a command prefix. Each module is configured in its `[launcher.modules.<module>]` section (`[launcher.views.<module>]` still works).

### Wallpaper Module

Browse and apply wallpapers from a directory. Optionally generates a color scheme from the wallpaper using Matugen.

**Invoke with:** `;wp`

| Option                    | Type     | Default                | Description                                   |
| ------------------------- | -------- | ---------------------- | --------------------------------------------- |
| `prefix`                  | `string` | `";wp"`               | Command prefix to invoke the wallpaper module. |
| `directory`               | `string` | `"~/Pictures/Wallpapers"` | Directory containing wallpaper images.        |
| `columns`                 | `integer` | `3`                   | Thumbnail cards per grid row.                 |
| `matugen`                 | `bool`   | `true`                 | After a wallpaper is applied, generate dark and light themes from it in the background and offer them above the grid. Nothing is applied until you pick one. |
| `matugen_scheme`          | `string` | `"scheme-tonal-spot"`  | matugen `--type` to generate with.            |

Thumbnails are decoded once and cached in `$XDG_CACHE_HOME/gpuishell/thumbs/`.

#### Example

```toml
[launcher.modules.wallpaper]
prefix = ";wp"
directory = "~/Pictures/Wallpapers"
```

### Services Module

View the health status of all system services (Audio, Network, Bluetooth, UPower, MPRIS, Notifications, Tray, Sysinfo, Privacy, Wallpaper, and Brightness).

**Invoke with:** `;s`

| Option    | Type     | Default | Description                            |
| --------- | -------- | ------- | -------------------------------------- |
| `prefix`  | `string` | `";s"`  | Command prefix to invoke the services module. |

#### Example

```toml
[launcher.modules.services]
prefix = ";s"
```

### Config Module

Edit `config.toml` from the launcher. Every section is a list of settings: Enter toggles a boolean, opens a choice list for an enum, or turns the search line into the value for numbers and text. The selected setting's detail pane shows its key and default plus a mouse control (switch, slider, buttons). The bar's **Layout** entry opens a custom editor to reorder widgets, move them between slots, add or remove them.

Edits apply immediately so you can preview them, and are kept in memory until you save with `Ctrl+S` (or discard with `Ctrl+Shift+D`). Saving rewrites only the changed keys through `toml_edit`, so comments and formatting are preserved; a file that no longer parses is never written.

**Invoke with:** `;cfg`

| Option    | Type     | Default  | Description                              |
| --------- | -------- | -------- | ---------------------------------------- |
| `prefix`  | `string` | `";cfg"` | Command prefix to invoke the config module. |

#### Example

```toml
[launcher.modules.config]
prefix = ";cfg"
```

### Config Module

Edit `config.toml` from the launcher. The root lists the config sections; selecting one shows its whole form in the detail pane (switches, sliders, choice buttons), and `Tab` lists every setting's keyboard action, so most edits need no navigation. `Enter` opens the section as rows grouped by topic, where `Enter` toggles a boolean, picks from a list, or turns the search line into the value for numbers and text. The bar's **Layout** entry opens a custom editor to reorder widgets, move them between slots, add or remove them.

Edits apply immediately so you can preview them, and stay in memory until you save with `Ctrl+S` (or discard with `Ctrl+Shift+D`). Saving rewrites only the changed keys, so comments and formatting are preserved; a file that no longer parses is never written.

**Invoke with:** `;cfg`

| Option    | Type     | Default  | Description                                 |
| --------- | -------- | -------- | ------------------------------------------- |
| `prefix`  | `string` | `";cfg"` | Command prefix to invoke the config module. |

#### Example

```toml
[launcher.modules.config]
prefix = ";cfg"
```

## Keyboard Shortcuts

The launcher supports vim-style navigation via Ctrl key combinations alongside standard keys.

### Navigation

| Key              | Ctrl Alternative       | Action          |
| ---------------- | ---------------------- | --------------- |
| `Up`             | `Ctrl+K` / `Ctrl+P`   | Move up         |
| `Down`           | `Ctrl+J` / `Ctrl+N`   | Move down       |
| `Page Up`        | `Ctrl+U`              | Page up         |
| `Page Down`      | `Ctrl+D`              | Page down       |
| `Enter`          |                        | Confirm / Run   |
| `Shift+Enter`    |                        | Second action (e.g. run a shell command in the background) |
| `Escape`         |                        | Clear / Close   |

### Editing

| Key                | Ctrl Alternative       | Action              |
| ------------------ | ---------------------- | ------------------- |
| `Left`             | `Ctrl+H`              | Move cursor left (in a grid such as `;wp`, move the selection) |
| `Right`            | `Ctrl+L` / `Ctrl+F`   | Move cursor right   |
| `Ctrl+Left`        | `Ctrl+B`              | Move word left      |
| `Ctrl+Right`       | `Ctrl+W`              | Move word right     |
| `Backspace`        |                        | Delete character    |
| `Ctrl+Backspace`   |                        | Delete word back    |
| `Ctrl+A`           |                        | Select all          |
| `Shift+Left`       |                        | Select left         |
| `Shift+Right`      |                        | Select right        |
| `Ctrl+Shift+Left`  |                        | Select word left    |
| `Ctrl+Shift+Right` |                        | Select word right   |
