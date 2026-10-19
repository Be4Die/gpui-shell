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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

A module is a GPUI entity. `new` sets up whatever keeps it fresh, and
`BarWidget` renders the content in both orientations. The trait wraps it in
the standard chip, so spacing and borders match the other modules.

```rust
// crates/app/src/bar/modules/uptime/mod.rs
//! Uptime widget that shows how long the system has been running.

mod config;
pub use config::UptimeConfig;

use gpui::{AnyElement, Context, Window, prelude::*};
use std::time::Duration;
use ui::ActiveTheme;

use super::{BarWidget, style};
use crate::config::ActiveConfig;

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
    fn render_horizontal(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let prefix = &cx.config().bar.modules.uptime.prefix;
        let text = format!("{prefix} {}", self.label());
        style::bar_label(text, false, cx.theme().colors.text).into_any_element()
    }

    fn render_vertical(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let label = style::bar_label(self.label(), true, cx.theme().colors.text);
        style::vertical_text_line(label)
    }
}

impl Render for Uptime {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_bar_widget(window, cx)
    }
}
```

Use `cx.background_executor().timer(..)` for timers, never
`tokio::time::sleep`. GPUI tasks don't run on tokio.

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

Today, three files have to know about every module:

1. **`bar/modules/mod.rs`**: `mod uptime;` and
   `pub use uptime::{Uptime, UptimeConfig};`
2. **`bar/modules/registry.rs`**: import `Uptime`, then add a
   `Widget::Uptime(Entity<Uptime>)` variant, its arm in `render()`, and the
   config name in `create()`:

   ```rust
   "Uptime" => Some(Widget::Uptime(cx.new(Uptime::new))),
   ```
3. **`bar/config.rs`**: add `pub uptime: UptimeConfig` to `ModulesConfig`,
   which makes `[bar.modules.uptime]` load.

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
- Interactive modules: override `is_interactive()` to get the hover style,
  and open panels through `crate::panel` so only one is open at a time.
