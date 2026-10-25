//! Explicit opt-in smoke tests. They send only a synthetic readiness request.
//! API tests use credentials saved through Ledgeling, never print or copy keys.
use ledgeling::tasks::{
    providers::{HttpModel, ModelProvider},
    runtimes,
    types::{Connection, Message, ProviderKind},
    Cancel,
};

fn get_test_var(name: &str) -> Result<String, std::env::VarError> {
    let alt = if let Some(stripped) = name.strip_prefix("LEDGELING_") {
        format!("PERCH_{stripped}")
    } else if let Some(stripped) = name.strip_prefix("PERCH_") {
        format!("LEDGELING_{stripped}")
    } else {
        name.to_string()
    };
    std::env::var(name).or_else(|_| std::env::var(&alt))
}

fn model(kind: ProviderKind, variable: &str) {
    let config = Connection {
        kind,
        model: get_test_var(variable).expect(variable),
        endpoint: get_test_var("LEDGELING_TEST_ENDPOINT").unwrap_or_default(),
        ..Default::default()
    };
    assert!(
        HttpModel(config)
            .capabilities(&Cancel::default())
            .unwrap()
            .structured
    );
}

#[test]
#[ignore = "Requires saved OpenAI API key and LEDGELING_TEST_OPENAI_MODEL (or PERCH_TEST_OPENAI_MODEL); incurs usage"]
fn openai() {
    model(ProviderKind::Openai, "LEDGELING_TEST_OPENAI_MODEL")
}

#[test]
#[ignore = "Requires saved Anthropic API key and LEDGELING_TEST_ANTHROPIC_MODEL (or PERCH_TEST_ANTHROPIC_MODEL); incurs usage"]
fn anthropic() {
    model(ProviderKind::Anthropic, "LEDGELING_TEST_ANTHROPIC_MODEL")
}

#[test]
#[ignore = "Requires LEDGELING_TEST_ENDPOINT and LEDGELING_TEST_COMPATIBLE_MODEL (or PERCH_TEST_*)"]
fn compatible() {
    model(ProviderKind::Compatible, "LEDGELING_TEST_COMPATIBLE_MODEL")
}
/// The on-device path end to end: starts `fm serve`, runs the structured
/// readiness completion, and stops the server. Nothing leaves the machine, but
/// it is slow and holds the model resident, so it stays opt-in.
#[test]
#[ignore = "Requires macOS 27 on Apple silicon with `sudo fm license` accepted"]
fn apple_foundation() {
    use ledgeling::tasks::apple;
    assert!(
        apple::readiness().is_ready(),
        "{}",
        apple::readiness().detail()
    );
    let capabilities = HttpModel(Connection {
        kind: ProviderKind::Apple,
        ..Default::default()
    })
    .capabilities(&Cancel::default())
    .unwrap();
    assert_eq!(capabilities.models, vec![apple::MODEL.to_string()]);
    assert!(capabilities.structured, "{}", capabilities.detail);
    // The full action schema a folder task sends; `fm serve` rejects any
    // schema outside its dialect, so this is what proves the format is right.
    let tools = [
        "finish",
        "list_files",
        "read_file",
        "find_files",
        "duplicates",
        "changes",
        "search_web",
        "read_page",
    ];
    let reply = HttpModel(Connection {
        kind: ProviderKind::Apple,
        ..Default::default()
    })
    .act(
        &[Message {
            role: "user".into(),
            content: "List the files in my folder.".into(),
        }],
        &tools,
        &Cancel::default(),
    )
    .unwrap();
    let call = ledgeling::tasks::providers::parse_call(&reply).unwrap();
    assert!(tools.contains(&call.tool.as_str()), "{reply}");
    apple::shutdown();
}
#[test]
#[ignore = "Requires installed Codex runtime"]
fn codex_protocol() {
    let caps = runtimes::capabilities(&Connection {
        kind: ProviderKind::Codex,
        ..Default::default()
    })
    .unwrap();
    assert!(caps.advanced);
    assert!(caps.detail.contains("Codex App Server protocol connected."));
    // The status line reflects the actual session: either an account is named
    // or sign-in is genuinely outstanding — never a demand when it is done.
    assert!(
        caps.detail.contains("Signed in to ChatGPT as")
            || caps.detail.contains("ign in with ChatGPT")
    );
}
#[test]
#[ignore = "Requires saved OpenAI API key; model listing is free"]
fn openai_model_list() {
    let models = ledgeling::tasks::providers::list_models(&Connection {
        kind: ProviderKind::Openai,
        ..Default::default()
    })
    .unwrap();
    assert!(!models.is_empty());
}
#[test]
#[ignore = "Requires saved Anthropic API key; model listing is free"]
fn anthropic_model_list() {
    let models = ledgeling::tasks::providers::list_models(&Connection {
        kind: ProviderKind::Anthropic,
        ..Default::default()
    })
    .unwrap();
    assert!(!models.is_empty());
}
#[test]
#[ignore = "Requires running Ollama"]
fn ollama_model_list() {
    let models = ledgeling::tasks::providers::list_models(&Connection {
        kind: ProviderKind::Ollama,
        ..Default::default()
    })
    .unwrap();
    assert!(!models.is_empty());
}
#[test]
#[ignore = "Requires npm run prepare:sidecar; no provider request"]
fn claude_bridge() {
    assert!(
        runtimes::capabilities(&Connection {
            kind: ProviderKind::Claude,
            ..Default::default()
        })
        .unwrap()
        .advanced
    )
}
#[test]
#[ignore = "Requires saved Brave Search key; sends synthetic query"]
fn brave_search() {
    use ledgeling::tasks::search::{Brave, SearchProvider};
    assert!(!Brave
        .search(
            "Rust programming language official documentation",
            &Cancel::default()
        )
        .unwrap()
        .is_empty());
}
