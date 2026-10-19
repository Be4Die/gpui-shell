---
title: Adding a launcher view
description: Write a launcher view against the current, unstable Rust API.
---

> **Temporary guide.** This describes the `LauncherView` trait as it is
> today. It will change: [#TBD](https://github.com/andre-brandao/gpui-shell/issues)
> tracks stabilizing it ahead of a plugin system, and the v0.4 launcher work
> gives it `&mut self`, a real context and async matching. Until then, views
> live in-tree and are written in Rust.

We'll build a `Links` view: type `;l`, filter a list of saved links, press
Enter to open one.

```toml
[launcher.modules.links]
prefix = ";l"
links = [
  { name = "GPUi Shell", url = "https://github.com/andre-brandao/gpui-shell" },
  { name = "GPUI", url = "https://gpui.rs" },
]
```

## How a view works

The launcher owns the input, the window and the selection. A view is
picked by its prefix, and the text after the prefix is its `query`. Each
frame the launcher:

1. calls `update_matches` once, where the view filters and stores the result;
2. calls `match_count` and `render_item` for each visible row, which read
   that stored result;
3. calls `on_select` with the selected index on Enter.

Never filter inside `render_item` or `match_count`. They run once per row.

Views that aren't a list (shell, web) override `render_content` instead and
return one element for the whole body.

## 1. Create the view

Each view is a directory in `crates/app/src/launcher/modules/`.

```rust
// crates/app/src/launcher/modules/links/config.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// Shown in the list and matched against the query.
    pub name: String,
    /// Opened with `xdg-open`.
    pub url: String,
}

/// `[launcher.modules.links]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LinksConfig {
    /// Prefix that opens the view.
    pub prefix: String,
    /// Saved links, in display order.
    pub links: Vec<Link>,
}

impl Default for LinksConfig {
    fn default() -> Self {
        Self {
            prefix: ";l".into(),
            links: Vec::new(),
        }
    }
}
```

```rust
// crates/app/src/launcher/modules/links/mod.rs
//! Links view for opening saved URLs.

pub mod config;

use gpui::{AnyElement, App, div, prelude::*};
use ui::{
    Clickable, Color, IconName, Label, LabelCommon, ListItem, ListItemSpacing, TextSize, Toggleable,
};

use self::config::{Link, LinksConfig};
use crate::launcher::view::{LauncherView, ViewContext};

pub struct LinksView {
    prefix: String,
    links: Vec<Link>,
    /// Links matching the current query, refreshed by `update_matches`.
    matches: Vec<Link>,
}

impl LinksView {
    pub fn new(config: &LinksConfig) -> Self {
        Self {
            prefix: config.prefix.clone(),
            links: config.links.clone(),
            matches: Vec::new(),
        }
    }
}

fn open(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

impl LauncherView for LinksView {
    fn prefix(&self) -> &str {
        &self.prefix
    }

    fn name(&self) -> &'static str {
        "Links"
    }

    fn icon(&self) -> IconName {
        IconName::Link
    }

    fn description(&self) -> &'static str {
        "Open a saved link"
    }

    fn update_matches(&mut self, vx: &ViewContext, _cx: &App) {
        let query = vx.query.to_lowercase();
        self.matches = self
            .links
            .iter()
            .filter(|link| link.name.to_lowercase().contains(&query))
            .cloned()
            .collect();
    }

    fn match_count(&self, _vx: &ViewContext, _cx: &App) -> usize {
        self.matches.len()
    }

    fn render_item(
        &self,
        index: usize,
        selected: bool,
        _vx: &ViewContext,
        _cx: &App,
    ) -> AnyElement {
        let Some(link) = self.matches.get(index) else {
            return div().into_any_element();
        };
        let url = link.url.clone();

        ListItem::new(format!("link-{index}"))
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(selected)
            .on_click(move |_, _, _| open(&url))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(Label::new(link.name.clone()).size(TextSize::Default))
                    .child(
                        Label::new(link.url.clone())
                            .size(TextSize::Small)
                            .color(Color::Muted),
                    ),
            )
            .into_any_element()
    }

    /// Enter. Returning true closes the launcher.
    fn on_select(&self, index: usize, _vx: &ViewContext, _cx: &mut App) -> bool {
        match self.matches.get(index) {
            Some(link) => {
                open(&link.url);
                true
            }
            None => false,
        }
    }
}
```

Two things the current trait makes awkward, both fixed by the v0.4 rework:

- **Clicks don't go through `on_select`.** The row's `on_click` has to repeat
  the action, as above.
- **Methods other than `update_matches` take `&self`.** A view that needs to
  change its own state on input (a toggle, a sub-page) needs a `RefCell` or
  similar today.

Other optional methods: `render_header` and `render_footer` around the list,
`render_footer_bar` for key hints, `handle_input` to intercept keys,
`is_default` for the no-prefix view, and `show_in_help`.

## 2. Register it

1. **`launcher/modules/mod.rs`**: `pub mod links;`,
   `pub use links::LinksView;` and
   `Box::new(LinksView::new(&config.modules.links))` in `all_views`.
2. **`launcher/config.rs`**: add `pub links: LinksConfig` to
   `ModulesConfig`, which makes `[launcher.modules.links]` load.

Prefixes must be unique. Check `?` in the launcher for the ones taken.

## 3. Try it

```bash
cargo run -- --input ";l"
```

Before the PR, give every config field a doc comment. You don't need to edit
`reference/launcher.md`: the Claude Docs workflow (`claude-docs.yml`)
updates the reference pages from changed `config.rs` files when `dev` merges
into `main`. Keep the config in a file named `config.rs` so it's picked up.
