//! The closed list of reasons Ledgeling may talk to the network. Each scope is
//! off until the user switches it on, and names the only hosts it may reach.
//! Adding an online feature starts here: a new variant, its hosts, its default.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Browsing and downloading pets from the marketplace.
    Marketplace,
    /// Anonymous usage statistics. Always opt-in, and always remembered.
    Analytics,
}

impl Scope {
    pub const ALL: [Scope; 2] = [Scope::Marketplace, Scope::Analytics];

    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Marketplace => "marketplace",
            Scope::Analytics => "analytics",
        }
    }

    pub fn parse(name: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|s| s.as_str() == name)
    }

    pub fn label(self) -> &'static str {
        match self {
            Scope::Marketplace => "Pet marketplace",
            Scope::Analytics => "Anonymous usage statistics",
        }
    }

    /// Said to the user next to the switch: what is sent, and what is not.
    pub fn description(self) -> &'static str {
        match self {
            Scope::Marketplace => {
                "Lets Ledgeling reach the marketplace to browse, download and update pets. \
Nothing is sent until you press a button in the marketplace."
            }
            Scope::Analytics => {
                "Sends a few anonymous counts, such as which features are used. Never window \
titles, file names, app names or anything you type. You can see exactly what would be sent."
            }
        }
    }

    /// The only hosts this scope may contact, redirects included. Every entry
    /// is matched exactly, so a look-alike or a sub-domain is refused.
    ///
    /// The `.invalid` names are placeholders (that top-level domain can never
    /// resolve): replace them with the real server before shipping the feature.
    pub fn hosts(self) -> &'static [&'static str] {
        match self {
            Scope::Marketplace => &["marketplace.ledgeling.invalid"],
            Scope::Analytics => &["analytics.ledgeling.invalid"],
        }
    }

    /// A scope that may not be granted for one session only.
    pub fn always_remembered(self) -> bool {
        matches!(self, Scope::Analytics)
    }
}
