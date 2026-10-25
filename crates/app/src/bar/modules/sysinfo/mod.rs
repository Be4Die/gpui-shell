//! SysInfo widget showing CPU and memory usage in the bar.
//!
//! Clicking the widget opens the launcher's system info module.

use gpui::{AnyElement, Context, Window, prelude::*, px};
use services::SysInfoData;
use ui::{ActiveTheme, ButtonCommon, ButtonLike, ButtonStyle, Clickable, IconName};

mod config;
pub use config::SysInfoConfig;

use super::{BarWidget, style};
use crate::config::ActiveConfig;
use crate::launcher;
use crate::state::AppState;
use crate::state::watch;

/// SysInfo widget showing CPU and memory usage in the bar.
pub struct SysInfo {
    data: SysInfoData,
}

struct SysInfoStat {
    icon: IconName,
    text: String,
    color: gpui::Hsla,
}

impl SysInfo {
    /// Create a new SysInfo widget.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let subscriber = AppState::sysinfo(cx).clone();
        let initial_data = subscriber.get();

        watch(cx, subscriber.subscribe(), |this, data, cx| {
            this.data = data;
            cx.notify();
        });

        SysInfo { data: initial_data }
    }

    fn cpu_icon(&self) -> IconName {
        if self.data.cpu_usage >= 90 {
            IconName::Flame
        } else {
            IconName::Cpu
        }
    }

    fn memory_icon(&self) -> IconName {
        if self.data.memory_usage >= 90 {
            IconName::ArrowDownUp
        } else {
            IconName::MemoryStick
        }
    }

    fn usage_text(usage: u32, is_vertical: bool) -> String {
        style::compact_percent(usage, is_vertical)
    }

    fn stats(
        &self,
        theme: &ui::Theme,
        config: &SysInfoConfig,
        is_vertical: bool,
    ) -> Vec<SysInfoStat> {
        let mut stats = Vec::new();

        if config.show_cpu {
            let cpu_usage = self.data.cpu_usage;
            stats.push(SysInfoStat {
                icon: self.cpu_icon(),
                text: Self::usage_text(cpu_usage, is_vertical),
                color: theme.colors.status.from_percentage(cpu_usage),
            });
        }

        if config.show_memory {
            let memory_usage = self.data.memory_usage;
            stats.push(SysInfoStat {
                icon: self.memory_icon(),
                text: Self::usage_text(memory_usage, is_vertical),
                color: theme.colors.status.from_percentage(memory_usage),
            });
        }

        if config.show_temp
            && let Some(temp) = self.data.temperature
        {
            stats.push(SysInfoStat {
                icon: if temp >= 70 {
                    IconName::Flame
                } else {
                    IconName::Thermometer
                },
                text: if is_vertical {
                    temp.to_string()
                } else {
                    format!("{temp}°C")
                },
                color: theme.colors.status.from_temperature(temp),
            });
        }

        stats
    }

    fn render_stats(
        &mut self,
        cx: &mut Context<Self>,
        stats: Vec<SysInfoStat>,
        is_vertical: bool,
    ) -> AnyElement {
        ButtonLike::new("sysinfo-widget")
            .style(ButtonStyle::Transparent)
            .on_click(cx.listener(|_, _, _, cx| {
                let prefix = cx.config().launcher.modules.sysinfo.prefix.clone();
                launcher::toggle(Some(prefix), cx);
            }))
            .child(
                style::stack(is_vertical)
                    .justify_center()
                    .gap(px(style::CHIP_GAP))
                    .children(stats.into_iter().map(|stat| {
                        style::bar_stat(is_vertical, stat.icon, stat.text, stat.color)
                    })),
            )
            .into_any_element()
    }
}

impl BarWidget for SysInfo {
    fn is_interactive(&self) -> bool {
        true
    }

    fn render_vertical(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let config = &cx.config().bar.modules.sysinfo;
        let stats = self.stats(cx.theme(), config, true);
        self.render_stats(cx, stats, true)
    }

    fn render_horizontal(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let config = &cx.config().bar.modules.sysinfo;
        let stats = self.stats(cx.theme(), config, false);
        self.render_stats(cx, stats, false)
    }
}

impl Render for SysInfo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_bar_widget(window, cx)
    }
}
