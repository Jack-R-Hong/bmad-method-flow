//! Tests verifying that plugin-coding-pack correctly registers with the SDK
//! PluginRegistry and that the injection/tool pipeline works end-to-end.
//!
//! These tests simulate **server mode**: Pulse's plugin-loader calls `register()`,
//! merges capabilities into a shared `PluginRegistry`, and provider-claude-code
//! queries the registry for injections and tools.

use pulse_plugin_sdk::types::injection::InjectionQuery;
use pulse_plugin_sdk::types::llm::{ToolCall, ToolSensitivity};
use pulse_plugin_sdk::{match_skills, HookPoint, PluginRegistry};

// ── Registration structure ─────────────────────────────────────────────────

#[test]
fn register_returns_config_injector_and_tool_provider() {
    let registration = plugin_coding_pack::register();

    assert_eq!(registration.metadata.name, "plugin-coding-pack");

    let mut has_config_injector = false;
    let mut has_tool_provider = false;
    let mut has_agent_def_provider = false;

    for cap in &registration.capabilities {
        match cap {
            HookPoint::ConfigInjector(c) => {
                assert_eq!(c.injector_name(), "bmad-agent-injector");
                assert_eq!(c.priority(), 100);
                has_config_injector = true;
            }
            HookPoint::ToolProvider(p) => {
                assert_eq!(p.provider_name(), "bmad-coding-pack");
                has_tool_provider = true;
            }
            HookPoint::AgentDefinitionProvider(a) => {
                assert_eq!(a.provider_name(), "bmad-agent-registry");
                has_agent_def_provider = true;
            }
            _ => {}
        }
    }

    assert!(
        has_config_injector,
        "registration must include ConfigInjector"
    );
    assert!(has_tool_provider, "registration must include ToolProvider");
    assert!(
        has_agent_def_provider,
        "registration must include AgentDefinitionProvider"
    );
}

#[test]
fn register_capabilities_count() {
    let registration = plugin_coding_pack::register();
    assert_eq!(
        registration.capabilities.len(),
        3,
        "should have exactly 3 capabilities: ConfigInjector + ToolProvider + AgentDefinitionProvider"
    );
}

// ── Registry integration ───────────────────────────────────────────────────

fn build_test_registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    registry.register_builtins();
    let registration = plugin_coding_pack::register();
    registry
        .register(registration)
        .expect("plugin-coding-pack registration should succeed");
    registry
}

#[test]
fn registry_contains_bmad_config_injector() {
    let registry = build_test_registry();

    // Registry should have at least 2 injectors: MarkdownFileInjector + BmadAgentInjector
    assert!(
        registry.config_injector_count() >= 2,
        "registry should have MarkdownFileInjector + BmadAgentInjector, got {}",
        registry.config_injector_count()
    );

    // BmadAgentInjector should be present
    let names: Vec<&str> = registry
        .config_injectors()
        .iter()
        .map(|(_, c)| c.injector_name())
        .collect();
    assert!(
        names.contains(&"bmad-agent-injector"),
        "bmad-agent-injector not found in registry, got: {:?}",
        names
    );
}

#[test]
fn registry_contains_bmad_tool_provider() {
    let registry = build_test_registry();

    let tools = registry.collect_tools("claude-code", "default", &InjectionQuery::new());
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

    assert!(
        tool_names.contains(&"bmad_validate_pack"),
        "bmad_validate_pack not found in tools: {:?}",
        tool_names
    );
    assert!(
        tool_names.contains(&"bmad_list_workflows"),
        "bmad_list_workflows not found in tools: {:?}",
        tool_names
    );
    assert!(
        tool_names.contains(&"bmad_list_plugins"),
        "bmad_list_plugins not found in tools: {:?}",
        tool_names
    );
    assert!(
        tool_names.contains(&"bmad_data_query"),
        "bmad_data_query not found in tools: {:?}",
        tool_names
    );
}

#[test]
fn registry_tool_sensitivity_correct() {
    let registry = build_test_registry();
    let tools = registry.collect_tools("claude-code", "default", &InjectionQuery::new());

    let low_tools = [
        "bmad_validate_pack",
        "bmad_list_workflows",
        "bmad_list_plugins",
        "bmad_data_query",
        "bmad_board_list",
    ];
    let high_tools = ["bmad_auto_dev_next"];

    for tool in &tools {
        if tool.name.starts_with("bmad_") {
            let expected = if low_tools.contains(&tool.name.as_str()) {
                ToolSensitivity::Low
            } else if high_tools.contains(&tool.name.as_str()) {
                ToolSensitivity::High
            } else {
                ToolSensitivity::Medium
            };
            assert_eq!(
                tool.sensitivity, expected,
                "tool {} has wrong sensitivity",
                tool.name
            );
        }
    }
}

// ── ConfigInjector pipeline (simulating server-mode provider-claude-code) ──

#[tokio::test]
async fn injection_pipeline_returns_persona_for_bmad_agent() {
    let registry = build_test_registry();

    // Simulate what provider-claude-code's build_injection_query() does:
    let query = InjectionQuery::new()
        .with_provider_name("claude-code")
        .with_agent_name("bmad/architect")
        .with_step_type("claude-code");

    let injections = registry.run_config_injectors(&query).await;

    // Should have injections from both MarkdownFileInjector and BmadAgentInjector
    // BmadAgentInjector produces exactly 2 injections (persona + principles)
    let bmad_injections: Vec<_> = injections
        .iter()
        .filter(|i| i.source == "bmad-agent-injector")
        .collect();

    assert_eq!(
        bmad_injections.len(),
        2,
        "BmadAgentInjector should produce 2 injections for bmad/architect, got {}",
        bmad_injections.len()
    );

    // Verify priorities
    assert_eq!(bmad_injections[0].priority, 100);
    assert_eq!(bmad_injections[1].priority, 110);

    // Verify persona content contains Winston (architect's display name)
    assert!(
        bmad_injections[0].content.contains("Winston")
            || bmad_injections[0].content.contains("Architect")
            || bmad_injections[0].content.contains("architect"),
        "injection should contain architect persona, got: {}",
        &bmad_injections[0].content[..bmad_injections[0].content.len().min(200)]
    );
}

#[tokio::test]
async fn injection_pipeline_skips_non_bmad_agents() {
    let registry = build_test_registry();

    let query = InjectionQuery::new()
        .with_provider_name("claude-code")
        .with_agent_name("other/agent")
        .with_step_type("claude-code");

    let injections = registry.run_config_injectors(&query).await;

    // BmadAgentInjector should NOT contribute
    let bmad_injections: Vec<_> = injections
        .iter()
        .filter(|i| i.source == "bmad-agent-injector")
        .collect();

    assert_eq!(
        bmad_injections.len(),
        0,
        "BmadAgentInjector should not inject for non-bmad agents"
    );
}

#[tokio::test]
async fn injection_pipeline_no_agent_name_skips_bmad() {
    let registry = build_test_registry();

    // No agent_name set — simulates a direct provider-claude-code call
    let query = InjectionQuery::new()
        .with_provider_name("claude-code")
        .with_step_type("claude-code");

    let injections = registry.run_config_injectors(&query).await;

    let bmad_injections: Vec<_> = injections
        .iter()
        .filter(|i| i.source == "bmad-agent-injector")
        .collect();

    assert_eq!(
        bmad_injections.len(),
        0,
        "BmadAgentInjector should not inject when agent_name is None"
    );
}

#[tokio::test]
async fn injection_pipeline_all_9_agents_produce_injections() {
    let registry = build_test_registry();

    let agent_names = [
        "bmad/analyst",
        "bmad/architect",
        "bmad/dev",
        "bmad/pm",
        "bmad/qa",
        "bmad/quick-flow-solo-dev",
        "bmad/sm",
        "bmad/tech-writer",
        "bmad/ux-designer",
    ];

    for name in &agent_names {
        let query = InjectionQuery::new()
            .with_provider_name("claude-code")
            .with_agent_name(*name)
            .with_step_type("claude-code");

        let injections = registry.run_config_injectors(&query).await;
        let bmad_injections: Vec<_> = injections
            .iter()
            .filter(|i| i.source == "bmad-agent-injector")
            .collect();

        assert_eq!(
            bmad_injections.len(),
            2,
            "agent {name} should produce 2 injections, got {}",
            bmad_injections.len()
        );
    }
}

#[tokio::test]
async fn injection_pipeline_priority_ordering() {
    let registry = build_test_registry();

    let query = InjectionQuery::new()
        .with_provider_name("claude-code")
        .with_agent_name("bmad/dev")
        .with_step_type("claude-code");

    let injections = registry.run_config_injectors(&query).await;

    // Verify injections are ordered by priority (SDK guarantees this)
    let priorities: Vec<i32> = injections.iter().map(|i| i.priority).collect();
    let mut sorted = priorities.clone();
    sorted.sort();
    assert_eq!(
        priorities, sorted,
        "injections should be ordered by priority"
    );

    // MarkdownFileInjector (priority 50) should come before BmadAgentInjector (priority 100)
    if let (Some(md_pos), Some(bmad_pos)) = (
        injections
            .iter()
            .position(|i| i.source == "markdown-file-injector"),
        injections
            .iter()
            .position(|i| i.source == "bmad-agent-injector"),
    ) {
        assert!(
            md_pos < bmad_pos,
            "MarkdownFileInjector (priority 50) should come before BmadAgentInjector (priority 100)"
        );
    }
}

// ── ToolProvider pipeline (simulating server-mode tool dispatch) ────────────

#[tokio::test]
async fn tool_dispatch_validate_pack() {
    let registry = build_test_registry();

    let call = ToolCall {
        id: "test-1".into(),
        name: "bmad_validate_pack".into(),
        arguments: serde_json::json!({}),
    };

    let result = registry
        .dispatch_tool_call("bmad_validate_pack", call, &InjectionQuery::new())
        .await;
    assert!(
        result.is_ok(),
        "dispatch bmad_validate_pack should succeed: {:?}",
        result.err()
    );

    let tool_result = result.unwrap();
    assert!(!tool_result.is_error, "tool result should not be an error");

    // Verify valid JSON returned
    let parsed: serde_json::Value =
        serde_json::from_str(&tool_result.content).expect("tool result should be valid JSON");
    assert!(
        parsed.get("plugins_ok").is_some() || parsed.get("valid").is_some(),
        "validate-pack result should have plugins_ok or valid field"
    );
}

#[tokio::test]
async fn tool_dispatch_unknown_tool_returns_not_found() {
    let registry = build_test_registry();

    let call = ToolCall {
        id: "test-2".into(),
        name: "nonexistent_tool".into(),
        arguments: serde_json::json!({}),
    };

    let result = registry
        .dispatch_tool_call("nonexistent_tool", call, &InjectionQuery::new())
        .await;
    assert!(result.is_err(), "unknown tool should return error");
}

// ── Executor agent_name passthrough ────────────────────────────────────────

#[test]
fn executor_injection_query_contains_agent_name() {
    // Verify that the InjectionQuery constructed by the executor
    // contains the correct agent_name extracted from step config.
    // This simulates what execute_bmad_agent_step does.

    let query = InjectionQuery::new()
        .with_agent_name("bmad/architect")
        .with_step_type("agent");

    assert_eq!(query.agent_name.as_deref(), Some("bmad/architect"));
    assert_eq!(query.step_type.as_deref(), Some("agent"));
}

#[test]
fn executor_injection_query_serializes_for_rpc() {
    // Verify that InjectionQuery round-trips through JSON serialization.
    // The executor serializes it into RPC params as "injection_query".

    let query = InjectionQuery::new()
        .with_agent_name("bmad/dev")
        .with_provider_name("claude-code")
        .with_step_type("agent");

    let serialized = serde_json::to_value(&query).expect("InjectionQuery should serialize");

    assert_eq!(
        serialized.get("agent_name").and_then(|v| v.as_str()),
        Some("bmad/dev")
    );
    assert_eq!(
        serialized.get("provider_name").and_then(|v| v.as_str()),
        Some("claude-code")
    );

    // Round-trip: deserialize back
    let deserialized: InjectionQuery =
        serde_json::from_value(serialized).expect("InjectionQuery should deserialize");
    assert_eq!(deserialized.agent_name.as_deref(), Some("bmad/dev"));
}

#[test]
fn executor_rpc_params_include_injection_query() {
    // Simulate what the executor builds for provider-claude-code RPC.
    // Verify that injection_query is present in the parameters map.

    let injection_query = InjectionQuery::new()
        .with_agent_name("bmad/qa")
        .with_step_type("agent");

    let mut parameters = serde_json::Map::new();
    parameters.insert("model_tier".to_string(), serde_json::json!("balanced"));
    // This mirrors what executor.rs does:
    if let Ok(iq_val) = serde_json::to_value(&injection_query) {
        parameters.insert("injection_query".to_string(), iq_val);
    }

    let params_val = serde_json::Value::Object(parameters);

    // provider-claude-code would extract agent_name from this
    let iq = params_val
        .get("injection_query")
        .expect("injection_query should be in params");
    assert_eq!(
        iq.get("agent_name").and_then(|v| v.as_str()),
        Some("bmad/qa")
    );
}

// ── AgentDefinitionProvider pipeline ──────────────────────────────────────

#[test]
fn registry_contains_bmad_agent_definition_provider() {
    let registry = build_test_registry();

    assert!(
        registry.agent_definition_provider_count() >= 1,
        "registry should have at least 1 agent definition provider, got {}",
        registry.agent_definition_provider_count()
    );
}

#[test]
fn registry_collect_agents_returns_all_9_bmad_agents() {
    let registry = build_test_registry();
    let agents = registry.collect_agents(None);

    let bmad_agents: Vec<_> = agents
        .iter()
        .filter(|a| a.name.starts_with("bmad/"))
        .collect();
    assert_eq!(
        bmad_agents.len(),
        9,
        "should have 9 BMAD agents, got {}",
        bmad_agents.len()
    );
}

#[test]
fn registry_collect_agents_sorted_deterministic() {
    let registry = build_test_registry();
    let agents = registry.collect_agents(None);

    let bmad_names: Vec<&str> = agents
        .iter()
        .filter(|a| a.name.starts_with("bmad/"))
        .map(|a| a.name.as_str())
        .collect();

    let mut sorted = bmad_names.clone();
    sorted.sort();
    assert_eq!(bmad_names, sorted, "agents should be sorted alphabetically");
}

#[test]
fn registry_get_agent_by_name() {
    let registry = build_test_registry();

    let architect = registry.get_agent("bmad/architect", None);
    assert!(architect.is_some(), "bmad/architect should be found");
    let a = architect.unwrap();
    assert!(a.description.as_ref().unwrap().contains("Winston"));
    assert!(a.system_prompt.is_some());
    assert_eq!(a.model_tier.as_deref(), Some("balanced"));
}

#[test]
fn registry_get_agent_not_found() {
    let registry = build_test_registry();
    assert!(registry.get_agent("bmad/nonexistent", None).is_none());
    assert!(registry.get_agent("other/agent", None).is_none());
}

#[test]
fn registry_agents_have_skills_for_routing() {
    let registry = build_test_registry();
    let agents = registry.collect_agents(None);

    let bmad_agents: Vec<_> = agents
        .iter()
        .filter(|a| a.name.starts_with("bmad/"))
        .collect();

    for agent in &bmad_agents {
        assert!(
            agent.skills.is_some() && !agent.skills.as_ref().unwrap().is_empty(),
            "agent {} should have skills for routing",
            agent.name
        );
    }
}

#[test]
fn registry_skill_routing_finds_architect() {
    let registry = build_test_registry();
    let agents = registry.collect_agents(None);

    let result = match_skills(
        &["distributed systems".into(), "API design".into()],
        &agents,
        Some("balanced"),
    );
    assert!(
        result.is_some(),
        "should find an agent for distributed systems + API design"
    );
    assert_eq!(result.unwrap().name, "bmad/architect");
}

#[test]
fn registry_skill_routing_finds_dev() {
    let registry = build_test_registry();
    let agents = registry.collect_agents(None);

    let result = match_skills(&["code implementation".into()], &agents, Some("balanced"));
    assert!(
        result.is_some(),
        "should find an agent for code implementation"
    );
    assert_eq!(result.unwrap().name, "bmad/dev");
}

// ── Metadata ───────────────────────────────────────────────────────────────

#[test]
fn metadata_has_correct_name_and_version() {
    let meta = plugin_coding_pack::metadata();
    assert_eq!(meta.name, "plugin-coding-pack");
    assert!(!meta.version.is_empty());
}
