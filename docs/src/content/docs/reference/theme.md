---
title: Theme
description: Theme configuration reference.
---

The theme is loaded from:

- `$XDG_CONFIG_HOME/gpuishell/theme.toml`
- or `~/.config/gpuishell/theme.toml` (fallback)

`theme.toml` is written when you apply a theme from the launcher (`~`
module) or change one of its appearance knobs there. If it is missing the
default dark theme is used; if it does not parse, the last good theme stays
and a notification names the file. Hot reload picks up edits when
`watch_theme = true` in `config.toml`.

## Top-level keys

| Key | Default | Description |
|-----|---------|-------------|
| `name` | `"Default Dark"` | Display name of the scheme. |
| `font_size` | `13.0` | Base font size in px; every text size is a ratio of it. |
| `radius_scale` | `1.0` | Multiplier on every corner radius. `0.0` is square everywhere (pills, dots and switch knobs included), `1.0` the stock radii, larger is rounder, capped at `4.0`. |
| `opacity` | `1.0` | Opacity of every shell surface - bar, panels, launcher, OSD, notification popups, dock - from `0.0` to `1.0`. Each surface can override it in `config.toml` (`[bar] opacity`, `[launcher] opacity`, `[panels] opacity`, `[osd] opacity`, `[notification] opacity`, `[dock] opacity`). Text, icons and cards inside a surface stay opaque. |

Switching scheme (from the launcher, matugen or a wallpaper palette) keeps
`font_size`, `radius_scale`, `opacity` and your `[colors]` overrides.

## `[base16]`

The sixteen palette slots every semantic colour is derived from, `base00`
to `base0f`, as `#RRGGBB` or `#RRGGBBAA`. Light or dark is decided from
`base00`'s lightness.

```toml
[base16]
base00 = "#282828ff"   # background
base01 = "#383838ff"   # surfaces
base02 = "#484848ff"   # selection, borders
base03 = "#4c4c4cff"   # muted text
base04 = "#b8b8b8ff"
base05 = "#eeeeeeff"   # text
base06 = "#e8e8e8ff"
base07 = "#feffffff"
base08 = "#f43753ff"   # error
base09 = "#dc9656ff"   # warning
base0a = "#ffc24bff"
base0b = "#c9d05cff"   # success
base0c = "#73cef4ff"
base0d = "#b3deefff"   # accent, info
base0e = "#d3b987ff"
base0f = "#a16946ff"
```

## `[colors]`

Optional overrides of individual semantic tokens, layered on top of the
palette derivation. Only the keys you set are written back; a theme with no
overrides has no `[colors]` table.

Surfaces: `background`, `surface_background`,
`elevated_surface_background`. Borders: `border`, `border_variant`,
`border_focused`, `border_selected`, `border_disabled`,
`border_transparent`. Text: `text`, `text_muted`, `text_placeholder`,
`text_disabled`, `text_accent`. Icons: `icon`, `icon_muted`,
`icon_disabled`, `icon_accent`. Interactive fills: `element_background`,
`element_hover`, `element_active`, `element_selected`, `element_disabled`
and the `ghost_element_*` set. Highlight: `accent`.

Status colours live under `[colors.status]`: `info`, `success`, `warning`,
`error`, `hint`, `hidden`, `ignored`, each with `_background` and `_border`
variants.

```toml
[colors]
accent = "#ff7f50"

[colors.status]
warning = "#ffc24b"
```

## Example

```toml
name = "tender"
font_size = 13.0
radius_scale = 0.0   # a blocky, square shell
opacity = 0.85       # slightly translucent surfaces

[base16]
# ... sixteen slots as above
```

## Text sizes

Every text size is a ratio of `font_size`:

| Size | Ratio | At 13px |
|------|-------|---------|
| XSmall | 0.77 | 10 |
| Small | 0.85 | 11 |
| Default | 1.0 | 13 |
| Medium | 1.08 | 14 |
| Large | 1.23 | 16 |
| XLarge | 1.38 | 18 |

Bar icon sizes follow `bar.size`, not the font: see the bar reference.
