---
title: Adding a bar module
description: Write a bar module against the current, unstable Rust API.
---

> **Temporary guide.** This describes the bar module API as it is today. It
> will change: [#TBD](https://github.com/andre-brandao/gpui-shell/issues)
> tracks stabilizing it ahead of a plugin system. Until then, modules live
> in-tree and are written in Rust.

We'll build an `Uptime` module that shows how long the machine has been up,
with a configurable prefix:

```toml
[bar]
end = ["Uptime", "Clock"]

[bar.modules.uptime]
prefix = "up"
```

## 1. Create the module

Each module is a directory in `crates/app/src/bar/modules/` with a
`config.rs` and a `mod.rs`.

```rust
// crates/app/src/bar/modules/uptime/config.rs
use serde::{Deserialize, Serialize};

/// `[bar.modules.uptime]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UptimeConfig {
    /// Text shown before the uptime in a horizontal bar.
    pub prefix: String,
}

impl Default for UptimeConfig {
    fn default() -> Self {
        Self {
            prefix: "up".into(),
        }
    }
}
```

A module is a GPUI entity, not a view. `new` sets up whatever keeps it fresh,
`BarWidget` renders the content in both orientations, and `descriptor()` is
what the registry lists. The bar hands every render a `BarCtx`: the
orientation, the ink to draw with, and the sizes to lay out with. Take
colours and sizes from it, never from the theme or a literal `px()`, so the
widget looks right in every bar style and follows the theme's radius scale.

```rust
// crates/app/src/bar/modules/uptime/mod.rs
//! Uptime widget that shows how long the system has been running.

mod config;
pub use config::UptimeConfig;

use gpui::{AnyElement, Context, Window, prelude::*};
use std::time::Duration;
use ui::IconName;

use super::{BarCtx, BarWidget, BarWidgetDescriptor, BarWidgetMeta, boxed, style};
use crate::config::ActiveConfig;

pub const fn descriptor() -> BarWidgetDescriptor {
    BarWidgetDescriptor::new(
        BarWidgetMeta {
            name: "Uptime",          // what config.toml says
            aliases: &[],            // older spellings that should keep working
            label: "Uptime",         // what the config editor shows
            description: "How long the machine has been up.",
            icon: IconName::Timer,
        },
        |cx| boxed(cx, Uptime::new),
    )
}

pub struct Uptime {
    secs: u64,
}

impl Uptime {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    this.secs = read_uptime();
                    cx.notify();
                });
                // The entity is gone (bar closed or reloaded): stop the timer.
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        Uptime {
            secs: read_uptime(),
        }
    }

    fn label(&self) -> String {
        format!("{}h{:02}", self.secs / 3600, self.secs / 60 % 60)
    }
}

fn read_uptime() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split('.').next()?.parse().ok())
        .unwrap_or(0)
}

impl BarWidget for Uptime {
    fn render_horizontal(
        &mut self,
        ctx: &BarCtx,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let prefix = &cx.config().bar.modules.uptime.prefix;
        style::label(ctx, format!("{prefix} {}", self.label())).into_any_element()
    }

    fn render_vertical(
        &mut self,
        ctx: &BarCtx,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        style::text_line(ctx, style::label(ctx, self.label()))
    }
}
```

`style` has the shared building blocks: `stack` (a row or column for the
orientation), `label` / `label_in`, `icon` / `icon_in`, `stat` (an icon and
a value), `value` (a label placed the way the orientation wants) and
`divider`. For a status colour past a threshold use `ctx.ink.status`; the
rest of the time draw with `ctx.ink.text` and `ctx.ink.muted`.

Use `cx.background_executor().timer(..)` for timers, never
`tokio::time::sleep`. GPUI tasks don't run on tokio.

### Chrome

The bar wraps the content in its chip; the widget only says which kind.
Override `shell()` to return `BarWidgetShell::Group` for a strip of items
(workspaces, tray) or `BarWidgetShell::None` to draw nothing when there is
nothing to show, and `is_interactive()` to get the hover style on a widget
that opens something.

### Data from a service

If your module shows system state, read it from a service instead of polling.
Subscribe in `new` and keep a copy of the data
(from `bar/modules/battery/mod.rs`):

```rust
let upower = AppState::upower(cx).clone();
let initial = upower.get();

watch(cx, upower.subscribe(), |this, data, cx| {
    this.data = data;
    cx.notify();
});
```

If your module changes something, call
`AppState::x(cx).dispatch(Command)` from a click handler. Don't mutate
the state yourself.

## 2. Register it

1. **`bar/modules/mod.rs`**: `mod uptime;`, `pub use uptime::UptimeConfig;`
   and one line in `REGISTRY`:

   ```rust
   pub static REGISTRY: [BarWidgetDescriptor; 12] = [
       // ...
       uptime::descriptor(),
   ];
   ```

   The config parser, the layout editor and the "add widget" list all read
   `REGISTRY`, so nothing else has to know the name.
2. **`bar/config.rs`**: add `pub uptime: UptimeConfig` to `ModulesConfig`,
   which makes `[bar.modules.uptime]` load. To edit it from the launcher's
   config module, add an `Editable` impl and a `"Uptime"` arm to
   `widget_field` in `launcher/modules/config/sections.rs`.

## 3. Try it

```bash
cargo run
```

Add `"Uptime"` to one of `start`, `center` or `end` in your `config.toml`.
Config hot-reloads, so you can try both orientations by changing
`bar.position` while it runs.

Before the PR:

- Give every config field a doc comment. You don't need to edit
  `reference/bar.md`: the Claude Docs workflow (`claude-docs.yml`) updates
  the reference pages from changed `config.rs` files when `dev` merges into
  `main`. Keep the config in a file named `config.rs` so it's picked up.
- Add a story in `crates/story` if the module has states worth looking at
  (empty, error, long text).
- Interactive modules open panels through `ShellWindows::toggle_panel` so
  only one is open at a time.
