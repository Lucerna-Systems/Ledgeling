//! Providers the user sets up once on the AI providers page. Every picker only
//! chooses among these, so terms are accepted in one place and a picked
//! provider always has what it needs to run.

use super::{apple, providers, types::*};

/// Bump when `cloud_terms` changes materially; older acceptances stop counting.
pub const CLOUD_TERMS_VERSION: u32 = 1;
/// Bump when `agent_terms` changes materially; older acceptances stop counting.
pub const AGENT_TERMS_VERSION: u32 = 1;

pub fn label(kind: &ProviderKind) -> &'static str {
    match kind {
        ProviderKind::None => "No AI",
        ProviderKind::Ollama => "Ollama",
        ProviderKind::Openai => "OpenAI",
        ProviderKind::Anthropic => "Anthropic",
        ProviderKind::Compatible => "your custom endpoint",
        ProviderKind::Codex => "ChatGPT · Codex",
        ProviderKind::Claude => "Claude Code",
        ProviderKind::Apple => "Apple Intelligence",
    }
}

fn cloud_terms(name: &str) -> String {
    format!(
        "\
Ledgeling sends your requests to {name} with your own API key, together with \
anything you attach and, for file tasks, the contents of files the model chooses \
to read in the folders you give it. That content leaves this computer.

{name}'s own terms and data policies apply to everything sent, and usage is billed \
to your key. Ledgeling does not control what the model produces: output may be \
inaccurate or incomplete. You review every file change before it happens.

Ledgeling is provided as is, without warranty of any kind."
    )
}

fn agent_terms(name: &str) -> String {
    format!(
        "\
{name} runs as an agent on this computer, signed in to your own account. It reads \
your requests and the folders you give it, sends them to its provider, and can \
propose commands as well as file changes. That content leaves this computer.

You approve each step before it runs, but commands have no universal undo. Your \
account's own terms apply. Ledgeling does not control what the agent produces.

Ledgeling is provided as is, without warranty of any kind."
    )
}

/// The terms a provider needs accepted before it can be added, with their
/// version. Local Ollama has none: nothing leaves the machine.
pub fn terms(kind: &ProviderKind) -> Option<(u32, String)> {
    match kind {
        ProviderKind::Apple => Some((apple::TERMS_VERSION, apple::TERMS.into())),
        ProviderKind::Openai | ProviderKind::Anthropic | ProviderKind::Compatible => {
            Some((CLOUD_TERMS_VERSION, cloud_terms(label(kind))))
        }
        ProviderKind::Codex | ProviderKind::Claude => {
            Some((AGENT_TERMS_VERSION, agent_terms(label(kind))))
        }
        ProviderKind::None | ProviderKind::Ollama => None,
    }
}

fn required_version(kind: &ProviderKind) -> u32 {
    terms(kind).map_or(0, |(version, _)| version)
}

/// Whether `kind` is added and its terms are accepted at this build's version.
pub fn ready(config: &Connections, kind: &ProviderKind) -> bool {
    *kind == ProviderKind::None
        || config
            .providers
            .get(kind)
            .is_some_and(|s| s.terms_version >= required_version(kind))
}

pub fn not_ready(kind: &ProviderKind) -> String {
    if matches!(kind, ProviderKind::None) {
        return String::new();
    }
    let name = label(kind);
    format!("Set up {name} on the AI providers page first. Its terms are accepted there, once.")
}

/// Copies the provider's endpoint and executable onto a connection, and its
/// model when the connection has none of its own.
fn apply(setup: &ProviderSetup, connection: &mut Connection) {
    connection.endpoint = setup.endpoint.clone();
    connection.executable = setup.executable.clone();
    if connection.model.trim().is_empty() {
        connection.model = setup.model.clone();
    }
    if connection.kind == ProviderKind::Apple {
        connection.model = apple::MODEL.into();
    }
}

fn connections_mut(config: &mut Connections) -> impl Iterator<Item = &mut Connection> {
    std::iter::once(&mut config.active).chain(config.per_pet.values_mut())
}

/// Brings a config saved before the providers page up to date: every provider
/// already in use becomes an added one, keeping what it was configured with.
/// Only acceptances actually recorded carry over, so a cloud provider that was
/// consented to per request must still have its terms accepted once.
pub fn normalize(config: &mut Connections) {
    if config.apple_terms.accepted_version > 0 {
        let legacy = config.apple_terms.clone();
        config.providers.entry(ProviderKind::Apple).or_default();
        let setup = config.providers.get_mut(&ProviderKind::Apple).unwrap();
        if setup.terms_version < legacy.accepted_version {
            setup.terms_version = legacy.accepted_version;
            setup.accepted_at = legacy.accepted_at;
        }
        config.apple_terms = AppleTerms::default();
    }
    let in_use: Vec<Connection> = std::iter::once(&config.active)
        .chain(config.per_pet.values())
        .filter(|c| c.kind != ProviderKind::None)
        .cloned()
        .collect();
    for connection in in_use {
        config
            .providers
            .entry(connection.kind.clone())
            .or_insert_with(|| ProviderSetup {
                model: connection.model.clone(),
                endpoint: connection.endpoint.clone(),
                executable: connection.executable.clone(),
                ..Default::default()
            });
    }
    if let Some(apple) = config.providers.get_mut(&ProviderKind::Apple) {
        apple.model = apple::MODEL.into();
    }
}

/// Validates a provider's settings and records it as added. `accept` is the
/// user ticking the terms on this build's text; an earlier acceptance at the
/// current version also counts, so editing a provider does not ask again.
pub fn add(
    config: &mut Connections,
    kind: ProviderKind,
    mut setup: ProviderSetup,
    accept: bool,
    now: i64,
) -> Result<(), String> {
    if kind == ProviderKind::None {
        return Err("Choose a provider to add.".into());
    }
    if kind == ProviderKind::Apple {
        setup.model = apple::MODEL.into();
    }
    if kind == ProviderKind::Codex && setup.executable.trim().is_empty() {
        setup.executable = "codex".into();
    }
    if matches!(
        kind,
        ProviderKind::Ollama
            | ProviderKind::Openai
            | ProviderKind::Anthropic
            | ProviderKind::Compatible
    ) {
        providers::base(&Connection {
            kind: kind.clone(),
            endpoint: setup.endpoint.clone(),
            ..Default::default()
        })?;
    }
    let previous = config.providers.get(&kind).cloned();
    let required = required_version(&kind);
    (setup.terms_version, setup.accepted_at) = match &previous {
        Some(p) if p.terms_version >= required => (p.terms_version, p.accepted_at),
        _ if required == 0 => (0, 0),
        _ if accept => (required, now),
        _ => return Err(format!("Accept the terms to add {}.", label(&kind))),
    };
    // Pets that were following the provider's old default model follow the
    // new one; a model someone picked for a single pet stays.
    let old_model = previous.map(|p| p.model).unwrap_or_default();
    for connection in connections_mut(config).filter(|c| c.kind == kind) {
        if connection.model == old_model {
            connection.model.clear();
        }
        apply(&setup, connection);
    }
    // Adding the first provider while nothing is picked makes it the default,
    // so the Assistant starts with the AI that was just set up.
    if config.active.kind == ProviderKind::None {
        config.active = Connection {
            kind: kind.clone(),
            ..Default::default()
        };
        apply(&setup, &mut config.active);
    }
    config.providers.insert(kind, setup);
    Ok(())
}

/// Removes a provider and every choice that pointed at it, so nothing is left
/// selected that could not run.
pub fn remove(config: &mut Connections, kind: &ProviderKind) {
    config.providers.remove(kind);
    if config.active.kind == *kind {
        config.active = Connection::default();
    }
    config.per_pet.retain(|_, c| c.kind != *kind);
}

/// Checks a config coming from the frontend against the stored one. The
/// frontend can only pick among added providers; it can never add one,
/// change a setup, or record an acceptance, so those always come from `stored`.
pub fn accept_choices(stored: &Connections, mut next: Connections) -> Result<Connections, String> {
    next.providers = stored.providers.clone();
    next.apple_terms = AppleTerms::default();
    for connection in connections_mut(&mut next) {
        if connection.kind == ProviderKind::None {
            continue;
        }
        let Some(setup) = stored.providers.get(&connection.kind) else {
            return Err(not_ready(&connection.kind));
        };
        apply(setup, connection);
    }
    // An added provider whose terms are still due can be picked; starting a
    // task is what waits for them, so unrelated saves are never blocked.
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(kind: ProviderKind, model: &str) -> Connection {
        Connection {
            kind,
            model: model.into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_provider_with_terms_cannot_be_added_without_accepting_them() {
        let mut config = Connections::default();
        let refused = add(
            &mut config,
            ProviderKind::Apple,
            ProviderSetup::default(),
            false,
            5,
        );
        assert!(refused.unwrap_err().contains("Accept the terms"));
        assert!(!ready(&config, &ProviderKind::Apple));
        add(
            &mut config,
            ProviderKind::Apple,
            ProviderSetup::default(),
            true,
            5,
        )
        .unwrap();
        assert!(ready(&config, &ProviderKind::Apple));
        assert_eq!(config.providers[&ProviderKind::Apple].accepted_at, 5);
    }

    #[test]
    fn the_first_provider_added_becomes_the_default() {
        let mut config = Connections::default();
        add(
            &mut config,
            ProviderKind::Apple,
            ProviderSetup::default(),
            true,
            1,
        )
        .unwrap();
        assert_eq!(config.active.kind, ProviderKind::Apple);
        assert_eq!(config.active.model, apple::MODEL);
        // A second one does not take over a default that is already set.
        add(
            &mut config,
            ProviderKind::Ollama,
            ProviderSetup {
                model: "llama".into(),
                ..Default::default()
            },
            false,
            1,
        )
        .unwrap();
        assert_eq!(config.active.kind, ProviderKind::Apple);
    }

    #[test]
    fn editing_a_provider_keeps_its_acceptance_and_moves_followers_to_the_new_model() {
        let mut config = Connections::default();
        let setup = |model: &str| ProviderSetup {
            model: model.into(),
            ..Default::default()
        };
        add(&mut config, ProviderKind::Anthropic, setup("old"), true, 1).unwrap();
        config
            .per_pet
            .insert(3, with(ProviderKind::Anthropic, "old"));
        config
            .per_pet
            .insert(4, with(ProviderKind::Anthropic, "picked"));
        add(&mut config, ProviderKind::Anthropic, setup("new"), false, 9).unwrap();
        assert_eq!(config.providers[&ProviderKind::Anthropic].accepted_at, 1);
        assert_eq!(config.active.model, "new");
        assert_eq!(config.per_pet[&3].model, "new");
        assert_eq!(config.per_pet[&4].model, "picked");
    }

    #[test]
    fn the_frontend_can_pick_but_never_add_or_accept() {
        let mut stored = Connections::default();
        add(
            &mut stored,
            ProviderKind::Ollama,
            ProviderSetup {
                model: "llama".into(),
                ..Default::default()
            },
            false,
            1,
        )
        .unwrap();
        let mut forged = stored.clone();
        forged.active = with(ProviderKind::Apple, "");
        forged.providers.insert(
            ProviderKind::Apple,
            ProviderSetup {
                terms_version: 99,
                ..Default::default()
            },
        );
        let refused = accept_choices(&stored, forged).err().unwrap();
        assert!(refused.contains("AI providers page"), "{refused}");
        let mut pick = stored.clone();
        pick.per_pet.insert(2, with(ProviderKind::Ollama, ""));
        let saved = accept_choices(&stored, pick).unwrap();
        assert_eq!(saved.per_pet[&2].model, "llama");
    }

    #[test]
    fn removing_a_provider_clears_every_choice_that_used_it() {
        let mut config = Connections::default();
        add(
            &mut config,
            ProviderKind::Apple,
            ProviderSetup::default(),
            true,
            1,
        )
        .unwrap();
        config.per_pet.insert(3, with(ProviderKind::Apple, ""));
        remove(&mut config, &ProviderKind::Apple);
        assert_eq!(config.active.kind, ProviderKind::None);
        assert!(config.per_pet.is_empty());
        assert!(!ready(&config, &ProviderKind::Apple));
    }

    #[test]
    fn legacy_configs_keep_what_was_in_use_and_only_real_acceptances() {
        let mut config = Connections {
            active: with(ProviderKind::Apple, ""),
            apple_terms: AppleTerms {
                accepted_version: apple::TERMS_VERSION,
                accepted_at: 7,
            },
            ..Default::default()
        };
        config.per_pet.insert(1, with(ProviderKind::Openai, "gpt"));
        config
            .per_pet
            .insert(2, with(ProviderKind::Ollama, "llama"));
        normalize(&mut config);
        assert!(ready(&config, &ProviderKind::Apple));
        assert_eq!(config.providers[&ProviderKind::Apple].accepted_at, 7);
        assert!(ready(&config, &ProviderKind::Ollama));
        // Consent to a cloud provider used to be given per request, so it
        // was never recorded and cannot be assumed now.
        assert!(config.providers.contains_key(&ProviderKind::Openai));
        assert!(!ready(&config, &ProviderKind::Openai));
    }

    #[test]
    fn providers_serialise_as_a_map_keyed_by_kind() {
        let mut config = Connections::default();
        add(
            &mut config,
            ProviderKind::Apple,
            ProviderSetup::default(),
            true,
            1,
        )
        .unwrap();
        let json = serde_json::to_value(&config).unwrap();
        assert!(json["providers"]["apple"].is_object());
        assert!(json.get("appleTerms").is_none());
        let back: Connections = serde_json::from_value(json).unwrap();
        assert!(ready(&back, &ProviderKind::Apple));
    }
}
