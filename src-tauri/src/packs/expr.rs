//! The condition grammar from the pack spec, and nothing else:
//!
//! ```text
//! expr       := or
//! or         := and ("||" and)*
//! and        := comparison ("&&" comparison)*
//! comparison := signal OP value
//! OP         := ">" | "<" | ">=" | "<=" | "==" | "!="
//! ```
//!
//! There is no function call, no property access, no assignment and no
//! evaluation of anything that is not one of the fixed signals below, so a
//! malicious pack has no path to executing code.
use crate::awareness::AppCategory;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    IdleSeconds,
    SessionMinutes,
    AppCategory,
    Hour,
    Energy,
    Mood,
    SurfaceCount,
    CursorDistance,
    CursorSpeed,
    IsDragging,
}
impl Signal {
    pub const ALL: [Signal; 10] = [
        Signal::IdleSeconds,
        Signal::SessionMinutes,
        Signal::AppCategory,
        Signal::Hour,
        Signal::Energy,
        Signal::Mood,
        Signal::SurfaceCount,
        Signal::CursorDistance,
        Signal::CursorSpeed,
        Signal::IsDragging,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Signal::IdleSeconds => "idle_seconds",
            Signal::SessionMinutes => "session_minutes",
            Signal::AppCategory => "app_category",
            Signal::Hour => "hour",
            Signal::Energy => "energy",
            Signal::Mood => "mood",
            Signal::SurfaceCount => "surface_count",
            Signal::CursorDistance => "cursor_distance",
            Signal::CursorSpeed => "cursor_speed",
            Signal::IsDragging => "is_dragging",
        }
    }
    fn parse(name: &str) -> Option<Signal> {
        Signal::ALL.into_iter().find(|s| s.as_str() == name)
    }
    fn kind(self) -> Kind {
        match self {
            Signal::AppCategory => Kind::Category,
            Signal::IsDragging => Kind::Bool,
            _ => Kind::Number,
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Number,
    Category,
    Bool,
}

/// Everything a condition may read. Values the engine owns; packs never set them.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Signals {
    pub idle_seconds: f64,
    pub session_minutes: f64,
    pub app_category: AppCategory,
    pub hour: f64,
    pub energy: f64,
    pub mood: f64,
    pub surface_count: f64,
    pub cursor_distance: f64,
    /// Pointer speed in logical pixels per second, smoothed over a few ticks.
    pub cursor_speed: f64,
    pub is_dragging: bool,
}
impl Default for Signals {
    fn default() -> Self {
        Self {
            idle_seconds: 0.,
            session_minutes: 0.,
            app_category: AppCategory::Other,
            hour: 12.,
            energy: 1.,
            mood: 0.7,
            surface_count: 0.,
            cursor_distance: f64::INFINITY,
            cursor_speed: 0.,
            is_dragging: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Comparison {
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
}
impl Comparison {
    fn parse(op: &str) -> Option<Comparison> {
        Some(match op {
            ">" => Comparison::Gt,
            "<" => Comparison::Lt,
            ">=" => Comparison::Ge,
            "<=" => Comparison::Le,
            "==" => Comparison::Eq,
            "!=" => Comparison::Ne,
            _ => return None,
        })
    }
    fn apply(self, a: f64, b: f64) -> bool {
        match self {
            Comparison::Gt => a > b,
            Comparison::Lt => a < b,
            Comparison::Ge => a >= b,
            Comparison::Le => a <= b,
            Comparison::Eq => (a - b).abs() < f64::EPSILON,
            Comparison::Ne => (a - b).abs() >= f64::EPSILON,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
enum Operand {
    Number(f64),
    Category(AppCategory),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Test {
    signal: Signal,
    op: Comparison,
    value: Operand,
}

/// A parsed condition: `||` of `&&` groups, matching the usual precedence.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Condition {
    source: String,
    groups: Vec<Vec<Test>>,
}

impl Condition {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn parse(text: &str) -> Result<Condition, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(
                "Expected a condition such as \"idle_seconds > 300\", found an empty string."
                    .into(),
            );
        }
        let mut groups = Vec::new();
        for or_part in split_on(trimmed, "||") {
            let mut group = Vec::new();
            for and_part in split_on(&or_part, "&&") {
                group.push(parse_test(and_part.trim())?);
            }
            groups.push(group);
        }
        Ok(Condition {
            source: trimmed.to_string(),
            groups,
        })
    }
    pub fn evaluate(&self, signals: &Signals) -> bool {
        if self.groups.is_empty() {
            return true;
        }
        self.groups
            .iter()
            .any(|group| group.iter().all(|test| test.evaluate(signals)))
    }
}

impl Test {
    fn evaluate(&self, s: &Signals) -> bool {
        match self.value {
            Operand::Number(v) => {
                let actual = match self.signal {
                    Signal::IdleSeconds => s.idle_seconds,
                    Signal::SessionMinutes => s.session_minutes,
                    Signal::Hour => s.hour,
                    Signal::Energy => s.energy,
                    Signal::Mood => s.mood,
                    Signal::SurfaceCount => s.surface_count,
                    Signal::CursorDistance => s.cursor_distance,
                    Signal::CursorSpeed => s.cursor_speed,
                    _ => return false,
                };
                self.op.apply(actual, v)
            }
            Operand::Category(v) => {
                let same = s.app_category == v;
                matches!(self.op, Comparison::Eq) == same
            }
            Operand::Bool(v) => {
                let same = s.is_dragging == v;
                matches!(self.op, Comparison::Eq) == same
            }
        }
    }
}

fn split_on(text: &str, separator: &str) -> Vec<String> {
    text.split(separator).map(|p| p.to_string()).collect()
}

fn parse_test(text: &str) -> Result<Test, String> {
    if text.is_empty() {
        return Err("Expected a comparison between two of `&&`/`||`, found nothing.".into());
    }
    let (name, op, value) = split_comparison(text)?;
    let signal = Signal::parse(name).ok_or_else(|| {
        format!(
            "Unknown signal `{name}`. Available signals: {}.",
            Signal::ALL
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let operand = match signal.kind() {
        Kind::Number => Operand::Number(
            value
                .parse::<f64>()
                .map_err(|_| format!("`{name}` compares against a number, found `{value}`."))?,
        ),
        Kind::Category => {
            if !matches!(op, Comparison::Eq | Comparison::Ne) {
                return Err(format!(
                    "`{name}` can only be compared with `==` or `!=`, found `{}`.",
                    op_text(op)
                ));
            }
            Operand::Category(AppCategory::parse(value).ok_or_else(|| {
                format!(
                    "Unknown app category `{value}`. Expected one of: {}.",
                    AppCategory::ALL
                        .iter()
                        .map(|c| c.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?)
        }
        Kind::Bool => {
            if !matches!(op, Comparison::Eq | Comparison::Ne) {
                return Err(format!(
                    "`{name}` can only be compared with `==` or `!=`, found `{}`.",
                    op_text(op)
                ));
            }
            Operand::Bool(match value {
                "true" => true,
                "false" => false,
                other => {
                    return Err(format!(
                        "`{name}` compares against true or false, found `{other}`."
                    ))
                }
            })
        }
    };
    Ok(Test {
        signal,
        op,
        value: operand,
    })
}

fn op_text(op: Comparison) -> &'static str {
    match op {
        Comparison::Gt => ">",
        Comparison::Lt => "<",
        Comparison::Ge => ">=",
        Comparison::Le => "<=",
        Comparison::Eq => "==",
        Comparison::Ne => "!=",
    }
}

fn split_comparison(text: &str) -> Result<(&str, Comparison, &str), String> {
    for op in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some(at) = text.find(op) {
            let (left, rest) = text.split_at(at);
            let right = &rest[op.len()..];
            let (left, right) = (left.trim(), right.trim());
            if left.is_empty() || right.is_empty() {
                return Err(format!(
                    "Expected `signal {op} value` in `{text}`, but one side is empty."
                ));
            }
            return Ok((left, Comparison::parse(op).unwrap_or(Comparison::Eq), right));
        }
    }
    Err(format!(
        "Expected a comparison such as `idle_seconds > 300` in `{text}`. \
Conditions must use one of > < >= <= == !=."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn signals() -> Signals {
        Signals {
            idle_seconds: 400.,
            session_minutes: 60.,
            app_category: AppCategory::Code,
            hour: 23.,
            energy: 0.5,
            ..Default::default()
        }
    }
    #[test]
    fn evaluates_and_or_precedence() {
        let c = Condition::parse("session_minutes > 50 && app_category == code").unwrap();
        assert!(c.evaluate(&signals()));
        let c = Condition::parse("hour >= 22 || hour < 6").unwrap();
        assert!(c.evaluate(&signals()));
        let c = Condition::parse("energy > 0.9 && mood > 0.9 || idle_seconds > 300").unwrap();
        assert!(c.evaluate(&signals()));
    }
    #[test]
    fn category_inequality_and_booleans() {
        assert!(Condition::parse("app_category != media")
            .unwrap()
            .evaluate(&signals()));
        assert!(Condition::parse("is_dragging == false")
            .unwrap()
            .evaluate(&signals()));
    }
    #[test]
    fn unknown_signal_names_the_alternatives() {
        let error = Condition::parse("idle_second > 5").unwrap_err();
        assert!(error.contains("Unknown signal `idle_second`"));
        assert!(error.contains("idle_seconds"));
    }
    #[test]
    fn type_mismatches_are_errors_not_silent_failures() {
        assert!(Condition::parse("idle_seconds > code").is_err());
        assert!(Condition::parse("app_category > code").is_err());
        assert!(Condition::parse("energy").is_err());
        assert!(Condition::parse("").is_err());
    }
}
