//! Applications module for searching and launching desktop applications.

pub mod config;

use gpui::{Context, EventEmitter};
use ui::IconName;

use self::config::AppsConfig;
use crate::launcher::item::{Item, ItemIcon};
use crate::launcher::module::{
    Items, LauncherModule, ModuleDescriptor, ModuleEvent, ModuleMeta, Outcome,
};
use crate::state::AppState;

const ICON: IconName = IconName::Layers;

pub fn descriptor(config: &AppsConfig) -> ModuleDescriptor {
    ModuleDescriptor::new(
        ModuleMeta {
            id: "apps",
            prefix: config.prefix.clone().into(),
            name: "Applications",
            icon: ICON,
            description: "Search and launch applications",
            default: true,
            in_help: true,
        },
        |_| AppsModule,
    )
}

pub struct AppsModule;

impl EventEmitter<ModuleEvent> for AppsModule {}

impl LauncherModule for AppsModule {
    fn items(&mut self, _query: &str, cx: &mut Context<Self>) -> Items {
        let apps = AppState::applications(cx).all();
        Items::Ready(
            apps.iter()
                .enumerate()
                .map(|(ix, app)| {
                    let icon = match &app.icon_path {
                        Some(path) => ItemIcon::Path(path.clone()),
                        None => ItemIcon::Icon(ICON.into()),
                    };
                    let item = Item::new(ix.to_string(), app.name.clone())
                        .icon(icon)
                        .action("launch", "Open");
                    match &app.description {
                        Some(description) => item.subtitle(description.clone()),
                        None => item,
                    }
                })
                .collect(),
        )
    }

    fn activate(&mut self, item: &Item, _action: &str, cx: &mut Context<Self>) -> Outcome {
        let apps = AppState::applications(cx).all();
        if let Some(app) = item.id.parse::<usize>().ok().and_then(|ix| apps.get(ix)) {
            app.launch();
        }
        Outcome::Close
    }
}
