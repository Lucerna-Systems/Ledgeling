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
    /// They are fixed when the app is built, from the environment variables
    /// `LEDGELING_MARKETPLACE_HOSTS` and `LEDGELING_ANALYTICS_HOSTS` (a comma
    /// separated list of bare host names, set per GitHub Environment). A build
    /// without them has no hosts, so the feature is unavailable: it cannot be
    /// switched on and nothing can connect.
    pub fn hosts(self) -> Vec<String> {
        parse_hosts(match self {
            Scope::Marketplace => option_env!("LEDGELING_MARKETPLACE_HOSTS"),
            Scope::Analytics => option_env!("LEDGELING_ANALYTICS_HOSTS"),
        })
    }

    /// Whether this build knows any server for the scope.
    pub fn available(self) -> bool {
        !self.hosts().is_empty()
    }

    /// A scope that may not be granted for one session only.
    pub fn always_remembered(self) -> bool {
        matches!(self, Scope::Analytics)
    }
}

/// A comma-separated list of bare host names, such as `market.example.com`.
/// Anything that is not one (a URL, a port, a path, an address, stray
/// characters) is dropped rather than guessed at, so a typo in a build setting
/// can only make a feature unavailable, never reach somewhere unintended.
pub fn parse_hosts(raw: Option<&str>) -> Vec<String> {
    let mut hosts: Vec<String> = raw
        .unwrap_or_default()
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| is_bare_host(h))
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts
}

fn is_bare_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        && host
            .split('.')
            .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
        // A name, not an IP address.
        && host.chars().any(|c| c.is_ascii_lowercase())
}
