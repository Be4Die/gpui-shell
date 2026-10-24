//! Calculator view: a custom body with no items. `= 2 * (3 + 4)` shows the
//! result as you type; Enter copies it.

pub mod config;

use gpui::{
    ClipboardItem, Context, EventEmitter, FontWeight, IntoElement, Render, SharedString, Window,
    div, prelude::*,
};
use ui::{ActiveTheme, Color, IconName, Label, LabelCommon, Radius, Spacing, TextSize};

use self::config::CalculatorConfig;
use crate::launcher::view::{
    Body, Hint, Items, LauncherView, NavKey, Outcome, ViewDescriptor, ViewEvent, ViewMeta,
};

pub fn descriptor(config: &CalculatorConfig) -> ViewDescriptor {
    ViewDescriptor::new(
        ViewMeta {
            id: "calculator",
            prefix: config.prefix.clone().into(),
            name: "Calculator",
            icon: IconName::Hash,
            description: "Evaluate arithmetic: + - * / % ^ ( )",
            default: false,
            in_help: true,
        },
        |_| CalculatorView {
            expression: String::new(),
            result: None,
        },
    )
}

pub struct CalculatorView {
    expression: String,
    result: Option<Result<f64, &'static str>>,
}

impl CalculatorView {
    fn formatted(&self) -> Option<String> {
        match self.result {
            Some(Ok(value)) => Some(format_number(value)),
            _ => None,
        }
    }
}

impl EventEmitter<ViewEvent> for CalculatorView {}

impl LauncherView for CalculatorView {
    fn items(&mut self, query: &str, cx: &mut Context<Self>) -> Items {
        self.expression = query.trim().to_string();
        self.result = (!self.expression.is_empty()).then(|| evaluate(&self.expression));
        cx.notify();
        Items::Ready(Vec::new())
    }

    fn activate(
        &mut self,
        _item: &crate::launcher::item::Item,
        _action: &str,
        _cx: &mut Context<Self>,
    ) -> Outcome {
        Outcome::Stay
    }

    fn body(&mut self, cx: &mut Context<Self>) -> Body {
        Body::Custom(cx.entity().into())
    }

    fn on_key(&mut self, key: NavKey, cx: &mut Context<Self>) -> Option<Outcome> {
        if key != NavKey::Enter {
            return None;
        }
        let value = self.formatted()?;
        cx.write_to_clipboard(ClipboardItem::new_string(value));
        Some(Outcome::Close)
    }

    fn footer_hints(&self, _selected: Option<&crate::launcher::item::Item>) -> Option<Vec<Hint>> {
        let mut hints: Vec<Hint> = Vec::new();
        if self.formatted().is_some() {
            hints.push(("Copy".into(), "Enter".into()));
        }
        hints.push(("Close".into(), "Esc".into()));
        Some(hints)
    }
}

impl Render for CalculatorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let (result, color): (SharedString, Color) = match self.result {
            None => (
                "Type an expression, e.g. 2 * (3 + 4)".into(),
                Color::Placeholder,
            ),
            Some(Ok(value)) => (format_number(value).into(), Color::Default),
            Some(Err(err)) => (err.into(), Color::Error),
        };

        div().p(Spacing::Large.pixels()).child(
            div()
                .w_full()
                .p(Spacing::XLarge.pixels())
                .rounded(Radius::Medium.pixels())
                .bg(colors.surface_background)
                .flex()
                .flex_col()
                .gap(Spacing::Medium.pixels())
                .child(
                    Label::new(if self.expression.is_empty() {
                        SharedString::from("=")
                    } else {
                        SharedString::from(self.expression.clone())
                    })
                    .size(TextSize::Small)
                    .color(Color::Muted),
                )
                .child(
                    div()
                        .text_size(TextSize::Large.rems())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color.hsla(colors))
                        .child(result),
                ),
        )
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        let text = format!("{value:.10}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Recursive descent over `+ - * / % ^`, parentheses, unary minus, `pi`
/// and `e`. `^` binds right and above unary minus, as on paper.
fn evaluate(input: &str) -> Result<f64, &'static str> {
    let mut parser = Parser {
        chars: input.chars().filter(|c| !c.is_whitespace()).collect(),
        pos: 0,
    };
    let value = parser.sum()?;
    if parser.pos != parser.chars.len() {
        return Err("Unexpected input");
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err("Not a finite number")
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(c);
        if found {
            self.pos += 1;
        }
        found
    }

    fn sum(&mut self) -> Result<f64, &'static str> {
        let mut value = self.product()?;
        loop {
            if self.eat('+') {
                value += self.product()?;
            } else if self.eat('-') {
                value -= self.product()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn product(&mut self) -> Result<f64, &'static str> {
        let mut value = self.unary()?;
        loop {
            if self.eat('*') || self.eat('x') {
                value *= self.unary()?;
            } else if self.eat('/') {
                value /= self.unary()?;
            } else if self.eat('%') {
                value %= self.unary()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn unary(&mut self) -> Result<f64, &'static str> {
        if self.eat('-') {
            return Ok(-self.unary()?);
        }
        self.eat('+');
        self.power()
    }

    fn power(&mut self) -> Result<f64, &'static str> {
        let base = self.atom()?;
        if self.eat('^') {
            return Ok(base.powf(self.unary()?));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<f64, &'static str> {
        if self.eat('(') {
            let value = self.sum()?;
            return if self.eat(')') {
                Ok(value)
            } else {
                Err("Missing )")
            };
        }
        let rest: String = self.chars[self.pos..].iter().collect();
        for (name, value) in [("pi", std::f64::consts::PI), ("e", std::f64::consts::E)] {
            if rest.starts_with(name) {
                self.pos += name.len();
                return Ok(value);
            }
        }
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || c == '.' || c == ',')
        {
            self.pos += 1;
        }
        let number: String = self.chars[start..self.pos]
            .iter()
            .filter(|c| **c != ',')
            .collect();
        number.parse().map_err(|_| "Not a number")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(evaluate("2 + 3 * 4"), Ok(14.0));
        assert_eq!(evaluate("(2 + 3) * 4"), Ok(20.0));
        assert_eq!(evaluate("2 ^ 3 ^ 2"), Ok(512.0));
        assert_eq!(evaluate("-2 ^ 2"), Ok(-4.0));
        assert_eq!(evaluate("10 % 4"), Ok(2.0));
    }

    #[test]
    fn constants_and_thousands_separators() {
        assert_eq!(evaluate("1,000 * 2"), Ok(2000.0));
        assert!((evaluate("2 * pi").unwrap() - std::f64::consts::TAU).abs() < 1e-12);
    }

    #[test]
    fn errors_are_reported() {
        assert_eq!(evaluate("(1 + 2"), Err("Missing )"));
        assert_eq!(evaluate("1 +"), Err("Not a number"));
        assert_eq!(evaluate("1 / 0"), Err("Not a finite number"));
    }

    #[test]
    fn results_are_formatted_without_noise() {
        assert_eq!(format_number(14.0), "14");
        assert_eq!(format_number(0.1 + 0.2), "0.3");
    }
}
