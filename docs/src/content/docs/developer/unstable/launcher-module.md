---
title: Adding a launcher module
description: Write a launcher module against the current, unstable Rust API.
---

> **Unstable.** The `LauncherPage` API can still change before 1.0, and
> modules live in-tree and are written in Rust. Pages describe declarative
> views (lists, grids, details, actions, navigation), so a future plugin API
> can take the same shape.

We'll build a `Links` module: type `;l`, filter a list of saved links, press
Enter to open one.

```toml
[launcher.modules.links]
prefix = ";l"
links = [
  { name = "GPUi Shell", url = "https://github.com/andre-brandao/gpui-shell" },
  { name = "GPUI", url = "https://gpui.rs" },
]
```

## How a module works

The launcher owns the window, the input, the selection, the drawing and the
keys. A module is a **page**: a GPUI entity implementing `LauncherPage`,
picked by its prefix. The text after the prefix is its query.

1. `content(query, cx)` returns what to show: a `List`, a `Grid`, a `Detail`
   or a `Custom` view. Lists and grids hold `Section`s of `Item`s: title,
   subtitle, icon, keywords, accessories, an optional `Detail`, and an
   `ActionPanel`.
2. The launcher fuzzy-ranks items within their sections, highlights the
   matched letters and draws only the rows on screen. Because it filters,
   it doesn't call `content` again on every keystroke, only when the page
   calls `cx.notify()`. A list with `.filtering(false)` is asked again on
   every edit and keeps the page's order.
3. Enter runs the first action, Shift+Enter the second, and Tab (or a right
   click) opens every action, filterable by typing. An action's
   `.shortcut("ctrl-r")` runs it directly. Each action returns an `Outcome`:
   `Close`, `Stay`, `Push(page)`, `Pop`, `PopToRoot` or `SetQuery(..)`.
4. A pushed page has its own search. Esc, or Backspace on an empty search,
   pops back with the input and selection restored.

When the page's own data changes (a service updated, a scan finished), it
calls `cx.notify()` and the launcher rebuilds its content, keeping the cursor
on the same item id. `content` must not notify.

Modules are built the first time their prefix is typed, so a module that
subscribes to a service in its constructor starts it only when someone opens
the module.

## 1. Create the module

Each module is a directory in `crates/app/src/launcher/modules/`.

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
    /// Prefix that opens the module.
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
//! Links module for opening saved URLs.

pub mod config;

use gpui::Context;
use ui::IconName;

use self::config::{Link, LinksConfig};
use crate::launcher::action::Action;
use crate::launcher::content::{Content, EmptyView, Item, List};
use crate::launcher::page::{LauncherPage, ModuleDescriptor, ModuleMeta};

pub fn descriptor(config: &LinksConfig) -> ModuleDescriptor {
    let links = config.links.clone();
    ModuleDescriptor::new(
        ModuleMeta {
            id: "links",
            prefix: config.prefix.clone().into(),
            name: "Links",
            icon: IconName::Link,
            description: "Open a saved link",
            default: false,
            in_help: true,
        },
        move |_cx| LinksModule { links: links.clone() },
    )
}

pub struct LinksModule {
    links: Vec<Link>,
}

impl LauncherPage for LinksModule {
    fn content(&mut self, _query: &str, _cx: &mut Context<Self>) -> Content {
        List::new()
            .empty(EmptyView::new("No links").description("Add some under [launcher.modules.links]"))
            .items(self.links.iter().map(|link| {
                Item::new(link.url.clone(), link.name.clone())
                    .subtitle(link.url.clone())
                    .icon(IconName::Link)
                    .action(Action::open(link.url.clone()))
                    .action(Action::copy(link.url.clone()).title("Copy URL"))
            }))
            .into()
    }
}
```

That's the whole module. Matching, highlighting, clicks, the actions panel,
the footer (`Open Enter · Copy URL Shift Enter`) and the empty state are the
launcher's job.

## Building blocks

All of these live in `launcher::content` and `launcher::action`.

| Want | Use |
| --- | --- |
| A list with sections | `List::new().section(Section::new("Title").items(..))`, `Item::new(id, title)` |
| Extras on the right of a row | `Item::accessory(Accessory::text(..) / ::tag(..) / ::icon(..))`, with `.color(..)` |
| A message when nothing matches | `List::empty(EmptyView::new(title).description(..).icon(..))` |
| A filter beside the search | `List::dropdown(Dropdown::new(value, cx, on_change).item(DropdownItem::new(value, title)))`; chips under the search, ← → while it is empty |
| The selected item's detail beside the list | `List::show_detail(true)` and `Item::detail(Detail)`; accessories are hidden while the pane shows |
| A grid of cards | `Grid::new(columns).aspect_ratio(16. / 9.)`; the item's icon (`Thumbnail`, `Path`, `Icon`, `Color`) is the cell |
| A full detail view | `Detail::new().markdown(..).metadata(Metadata::label / ::tags / ::link / ::Separator)` |
| Grouped actions | `ActionPanel::new().action(..).section("Title", [..])` on an item or a view |
| A custom action | `Action::new(title, \|window, cx\| ..)`, or `Action::listener(title, cx, \|this, window, cx\| ..)` to run on the page |
| Navigation | `Action::push(title, \|cx\| Page::new(cx))`, `Action::push_detail(title, \|cx\| detail)`, or return `Outcome::push(entity)` / `Outcome::Pop` |
| Copy or open | `Action::copy(text)`, `Action::open(url_or_path)` |
| Loading state, badge title, placeholder | `.loading(bool)`, `.title(..)`, `.placeholder(..)` on any view |
| Filter and order items yourself | `.filtering(false)`; `content` gets each new query |

Detail markdown is a block subset: `#`-`###` headings, paragraphs, `-` lists,
`>` quotes, fenced code, and `![alt](path)` images on their own line.

A page can also draw its own pieces:

| Want | Use |
| --- | --- |
| Your own row or card | `render_item`; the launcher still owns its click and selection |
| A summary or toggles above the body | `render_header`, with `cx.listener` for clicks |
| No list at all, fully interactive | `Custom::new(cx.entity())` and `impl Render`; give it `.actions(..)` so Enter still works |

For images in your own rows, `launcher::row::thumbnail_image` returns a
cached, downscaled thumbnail and notifies the owner entity when it lands.

References in-tree: `apps` (actions, `push_detail`), `services` (detail pane,
`Action::push`, a second page), `theme` (dropdown, sections, `render_item`),
`wallpaper` (grid), `shell` (`filtering(false)`) and `calculator` / `web`
(custom views).

## 2. Register it

1. **`launcher/modules/mod.rs`**: `pub mod links;` and
   `links::descriptor(&config.links)` in `registry`.
2. **`launcher/config.rs`**: add `pub links: LinksConfig` to `ModulesConfig`,
   which makes `[launcher.modules.links]` load.

Prefixes must be unique. Check `?` in the launcher for the ones taken.

## 3. Try it

```bash
cargo run -- --input ";l"
```

Before the PR, give every config field a doc comment. You don't need to edit
`reference/launcher.md`: the Claude Docs workflow (`claude-docs.yml`)
updates the reference pages from changed `config.rs` files when `dev` merges
into `main`. Keep the config in a file named `config.rs` so it's picked up.
