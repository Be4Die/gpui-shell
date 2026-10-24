//! Workspaces module for switching between compositor workspaces.

pub mod config;

use gpui::{Context, EventEmitter};
use services::CompositorCommand;
use ui::IconName;

use self::config::WorkspacesConfig;
use crate::launcher::item::{Item, ItemIcon};
use crate::launcher::module::{
    Items, LauncherModule, ModuleDescriptor, ModuleEvent, ModuleMeta, Outcome,
};
use crate::state::{AppState, watch};

const ICON: IconName = IconName::Layout;

pub fn descriptor(config: &WorkspacesConfig) -> ModuleDescriptor {
    ModuleDescriptor::new(
        ModuleMeta {
            id: "workspaces",
            prefix: config.prefix.clone().into(),
            name: "Workspaces",
            icon: ICON,
            description: "Switch between workspaces",
            default: false,
            in_help: true,
        },
        WorkspacesModule::new,
    )
}

pub struct WorkspacesModule;

impl WorkspacesModule {
    fn new(cx: &mut Context<Self>) -> Self {
        let compositor = AppState::compositor(cx).subscribe();
        watch(cx, compositor, |_, _, cx| {
            cx.emit(ModuleEvent::ItemsChanged)
        });
        Self
    }
}

impl EventEmitter<ModuleEvent> for WorkspacesModule {}

impl LauncherModule for WorkspacesModule {
    fn items(&mut self, _query: &str, cx: &mut Context<Self>) -> Items {
        let compositor = AppState::compositor(cx).get();
        Items::Ready(
            compositor
                .workspaces
                .into_iter()
                .filter(|ws| !ws.is_special)
                .map(|ws| {
                    let title = if ws.name.is_empty() {
                        format!("Workspace {}", ws.id)
                    } else {
                        ws.name.clone()
                    };
                    Item::new(ws.id.to_string(), title)
                        .subtitle(format!("{} windows on {}", ws.windows, ws.monitor))
                        .keywords(ws.monitor)
                        .icon(ItemIcon::Icon(ICON.into()))
                        .action("focus", "Switch")
                })
                .collect(),
        )
    }

    fn activate(&mut self, item: &Item, _action: &str, cx: &mut Context<Self>) -> Outcome {
        if let Ok(id) = item.id.parse() {
            AppState::compositor(cx).dispatch(CompositorCommand::FocusWorkspace(id));
        }
        Outcome::Close
    }
}
