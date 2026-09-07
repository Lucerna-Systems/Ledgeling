//! Every validation result names the file, the JSON path, what was wrong and
//! what was expected. No message is ever a bare serde error or a stack trace.
use serde::Serialize;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// The pack cannot load.
    Error,
    /// The pack loads but is probably not what the author meant.
    Warning,
    /// A suggestion, shown in Pet Lab only.
    Info,
}
impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Tier::Error => "ERROR",
            Tier::Warning => "WARN",
            Tier::Info => "INFO",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub tier: Tier,
    /// Pack-relative file, e.g. `pet.json` or `sprites/idle.png`.
    pub file: String,
    /// Dotted JSON path, e.g. `animations.climb.frames`. Empty for whole files.
    pub path: String,
    pub problem: String,
    pub expected: String,
}
impl Diagnostic {
    pub fn new(
        tier: Tier,
        file: impl Into<String>,
        path: impl Into<String>,
        problem: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Self {
            tier,
            file: file.into(),
            path: path.into(),
            problem: problem.into(),
            expected: expected.into(),
        }
    }
    pub fn error(
        file: impl Into<String>,
        path: impl Into<String>,
        problem: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Diagnostic::new(Tier::Error, file, path, problem, expected)
    }
    pub fn warning(
        file: impl Into<String>,
        path: impl Into<String>,
        problem: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Diagnostic::new(Tier::Warning, file, path, problem, expected)
    }
    pub fn info(
        file: impl Into<String>,
        path: impl Into<String>,
        problem: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Diagnostic::new(Tier::Info, file, path, problem, expected)
    }
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = if self.path.is_empty() {
            self.file.clone()
        } else {
            format!("{} → {}", self.file, self.path)
        };
        write!(
            f,
            "{:<6} {}\n       {}\n       {}",
            self.tier.label(),
            location,
            self.problem,
            self.expected
        )
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
}
impl Report {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.tier == Tier::Error)
    }
    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }
    pub fn extend(&mut self, other: Report) {
        self.diagnostics.extend(other.diagnostics);
    }
    /// A single human-readable block, for logs and the error banner.
    pub fn text(&self) -> String {
        self.diagnostics
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
