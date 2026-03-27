//! BMAD Agent Registry — exposes BMAD agents as `SdkAgentDefinition`s for
//! workspace-based discovery and skill routing.
//!
//! Loads agent data from the same CSV manifest used by `BmadAgentInjector`,
//! mapping each agent to an `SdkAgentDefinition` with skills derived from
//! the CSV `capabilities` column.

use pulse_plugin_sdk::traits::agent_definition::AgentDefinitionProvider;
use pulse_plugin_sdk::SdkAgentDefinition;
use std::collections::HashMap;
use std::path::Path;

/// Access control list for an agent in the mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentAcl {
    /// Agent names this agent can invoke
    pub can_invoke: Vec<String>,
    /// Agent names this agent can respond to
    pub can_respond_to: Vec<String>,
}

/// Agent data parsed from the CSV manifest.
#[derive(Debug, Clone)]
struct AgentEntry {
    sdk_name: String,
    display_name: String,
    title: String,
    role: String,
    identity: String,
    communication_style: String,
    principles: String,
    skills: Vec<String>,
}

/// Provides BMAD agent definitions to the Pulse registry for workspace-based
/// discovery and skill-based routing.
pub struct BmadAgentRegistry {
    agents: HashMap<String, AgentEntry>,
    /// Sorted agent names for deterministic list ordering.
    sorted_names: Vec<String>,
}

impl BmadAgentRegistry {
    /// Create a new registry by loading the agent manifest CSV.
    pub fn new(manifest_path: &Path) -> Self {
        let content = match std::fs::read_to_string(manifest_path) {
            Ok(c) => c,
            Err(_) => {
                tracing::warn!(
                    "BmadAgentRegistry: manifest not found at {}, no agents loaded",
                    manifest_path.display()
                );
                return Self {
                    agents: HashMap::new(),
                    sorted_names: Vec::new(),
                };
            }
        };

        let logical_rows = crate::config_injector::split_csv_rows(&content);

        let header = match logical_rows.first() {
            Some(h) => h.as_str(),
            None => {
                return Self {
                    agents: HashMap::new(),
                    sorted_names: Vec::new(),
                };
            }
        };

        let header_fields = crate::config_injector::parse_csv_row(header);
        let col = |name: &str| -> Option<usize> { header_fields.iter().position(|f| f == name) };

        let idx_name = match col("name") {
            Some(i) => i,
            None => {
                return Self {
                    agents: HashMap::new(),
                    sorted_names: Vec::new(),
                };
            }
        };
        let idx_display_name = col("displayName");
        let idx_title = col("title");
        let idx_role = col("role");
        let idx_identity = col("identity");
        let idx_communication_style = col("communicationStyle");
        let idx_principles = col("principles");
        let idx_capabilities = col("capabilities");

        let max_required = [
            Some(idx_name),
            idx_display_name,
            idx_title,
            idx_role,
            idx_identity,
            idx_communication_style,
            idx_principles,
            idx_capabilities,
        ]
        .iter()
        .filter_map(|i| *i)
        .max()
        .unwrap_or(idx_name);

        let mut agents = HashMap::new();

        for row in &logical_rows[1..] {
            if row.trim().is_empty() {
                continue;
            }

            let fields = crate::config_injector::parse_csv_row(row);
            if fields.len() <= max_required {
                continue;
            }

            let name = fields[idx_name].trim();
            if name.is_empty() {
                continue;
            }

            let sdk_name = format!("bmad/{name}");
            let get = |idx: Option<usize>| -> String {
                match idx {
                    Some(i) if i < fields.len() => fields[i].clone(),
                    _ => String::new(),
                }
            };

            let capabilities_str = get(idx_capabilities);
            let skills: Vec<String> = capabilities_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let entry = AgentEntry {
                sdk_name: sdk_name.clone(),
                display_name: get(idx_display_name),
                title: get(idx_title),
                role: get(idx_role),
                identity: get(idx_identity),
                communication_style: get(idx_communication_style),
                principles: get(idx_principles),
                skills,
            };

            agents.insert(sdk_name, entry);
        }

        let mut sorted_names: Vec<String> = agents.keys().cloned().collect();
        sorted_names.sort();

        Self {
            agents,
            sorted_names,
        }
    }

    /// Returns the number of loaded agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Get the full ACL for an agent by name.
    ///
    /// ACL rules are static and architecture-defined. Unknown agents receive
    /// conservative defaults (can invoke only `bmad/developer`).
    pub fn get_acl(&self, agent_name: &str) -> AgentAcl {
        let can_respond_to = vec!["bmad/pm".to_string(), "bmad/sm".to_string()];

        let can_invoke = match agent_name {
            "bmad/architect" => vec![
                "bmad/analyst".to_string(),
                "bmad/developer".to_string(),
                "bmad/ux-designer".to_string(),
            ],
            "bmad/qa" => vec!["bmad/developer".to_string()],
            "bmad/quick-flow-solo-dev" => vec![],
            _ => vec!["bmad/developer".to_string()],
        };

        AgentAcl {
            can_invoke,
            can_respond_to,
        }
    }

    /// Get the list of agents this agent can invoke.
    pub fn get_can_invoke(&self, agent_name: &str) -> Vec<String> {
        self.get_acl(agent_name).can_invoke
    }

    /// Get the list of agents this agent can respond to.
    pub fn get_can_respond_to(&self, agent_name: &str) -> Vec<String> {
        self.get_acl(agent_name).can_respond_to
    }
}

fn entry_to_definition(entry: &AgentEntry) -> SdkAgentDefinition {
    let system_prompt = format!(
        "You are {}, {}.\n\n## Identity\n{}\n\n## Communication Style\n{}\n\n## Role\n{}\n\n## Principles\n{}",
        entry.display_name, entry.title, entry.identity, entry.communication_style, entry.role, entry.principles
    );

    SdkAgentDefinition {
        name: entry.sdk_name.clone(),
        description: Some(format!("{} — {}", entry.display_name, entry.title)),
        system_prompt: Some(system_prompt),
        model_tier: Some("balanced".to_string()),
        model: None,
        tools: None,
        skills: if entry.skills.is_empty() {
            None
        } else {
            Some(entry.skills.clone())
        },
        max_tokens: None,
        temperature: None,
        max_tool_rounds: None,
    }
}

impl AgentDefinitionProvider for BmadAgentRegistry {
    fn provider_name(&self) -> &str {
        "bmad-agent-registry"
    }

    fn list_agents(&self, _workspace: Option<&str>) -> Vec<SdkAgentDefinition> {
        self.sorted_names
            .iter()
            .filter_map(|name| self.agents.get(name))
            .map(entry_to_definition)
            .collect()
    }

    fn get_agent(&self, name: &str, _workspace: Option<&str>) -> Option<SdkAgentDefinition> {
        self.agents.get(name).map(entry_to_definition)
    }
}

// Compile-time assertion: BmadAgentRegistry is Send + Sync.
const _: () = {
    fn _assert_send_sync<T: Send + Sync>() {}
    fn _check() {
        _assert_send_sync::<BmadAgentRegistry>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn test_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("_bmad/_config/agent-manifest.csv")
    }

    #[test]
    fn loads_all_9_agents() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(registry.agent_count(), 9);
    }

    #[test]
    fn provider_name() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(registry.provider_name(), "bmad-agent-registry");
    }

    #[test]
    fn list_agents_returns_sorted() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let agents = registry.list_agents(None);
        assert_eq!(agents.len(), 9);

        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "agents should be sorted alphabetically");
    }

    #[test]
    fn list_agents_all_have_skills() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let agents = registry.list_agents(None);

        for agent in &agents {
            assert!(
                agent.skills.is_some(),
                "agent {} should have skills",
                agent.name
            );
            assert!(
                !agent.skills.as_ref().unwrap().is_empty(),
                "agent {} should have non-empty skills",
                agent.name
            );
        }
    }

    #[test]
    fn list_agents_all_have_system_prompt() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let agents = registry.list_agents(None);

        for agent in &agents {
            assert!(
                agent.system_prompt.is_some(),
                "agent {} should have system_prompt",
                agent.name
            );
            assert!(
                !agent.system_prompt.as_ref().unwrap().is_empty(),
                "agent {} should have non-empty system_prompt",
                agent.name
            );
        }
    }

    #[test]
    fn get_agent_found() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let agent = registry.get_agent("bmad/architect", None);
        assert!(agent.is_some());

        let a = agent.unwrap();
        assert_eq!(a.name, "bmad/architect");
        assert!(a.description.as_ref().unwrap().contains("Winston"));
        assert!(a.model_tier.as_ref().unwrap() == "balanced");
        assert!(a
            .skills
            .as_ref()
            .unwrap()
            .contains(&"distributed systems".to_string()));
    }

    #[test]
    fn get_agent_not_found() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert!(registry.get_agent("bmad/nonexistent", None).is_none());
        assert!(registry.get_agent("other/agent", None).is_none());
    }

    #[test]
    fn graceful_degradation() {
        let registry = BmadAgentRegistry::new(Path::new("/nonexistent/path.csv"));
        assert_eq!(registry.agent_count(), 0);
        assert!(registry.list_agents(None).is_empty());
        assert!(registry.get_agent("bmad/architect", None).is_none());
    }

    #[test]
    fn architect_skills_from_csv() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let agent = registry.get_agent("bmad/architect", None).unwrap();
        let skills = agent.skills.unwrap();
        assert!(skills.contains(&"distributed systems".to_string()));
        assert!(skills.contains(&"cloud infrastructure".to_string()));
        assert!(skills.contains(&"API design".to_string()));
        assert!(skills.contains(&"scalable patterns".to_string()));
    }

    #[test]
    fn trait_object_send_sync() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let _arc: Arc<dyn AgentDefinitionProvider> = Arc::new(registry);
    }

    #[test]
    fn skill_routing_integration() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let agents = registry.list_agents(None);

        // Should find architect for distributed systems
        let result = pulse_plugin_sdk::match_skills(
            &["distributed systems".into(), "API design".into()],
            &agents,
            Some("balanced"),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "bmad/architect");

        // Should find dev for code implementation
        let result = pulse_plugin_sdk::match_skills(
            &["code implementation".into()],
            &agents,
            Some("balanced"),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "bmad/dev");
    }

    // ── ACL tests ──────────────────────────────────────────────────────

    #[test]
    fn acl_architect_can_invoke() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_invoke("bmad/architect"),
            vec![
                "bmad/analyst".to_string(),
                "bmad/developer".to_string(),
                "bmad/ux-designer".to_string(),
            ]
        );
    }

    #[test]
    fn acl_architect_can_respond_to() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_respond_to("bmad/architect"),
            vec!["bmad/pm".to_string(), "bmad/sm".to_string()]
        );
    }

    #[test]
    fn acl_qa_can_invoke() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_invoke("bmad/qa"),
            vec!["bmad/developer".to_string()]
        );
    }

    #[test]
    fn acl_quick_flow_solo_dev_can_invoke_empty() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let can_invoke = registry.get_can_invoke("bmad/quick-flow-solo-dev");
        assert!(
            can_invoke.is_empty(),
            "quick-flow-solo-dev should have empty can_invoke"
        );
    }

    #[test]
    fn acl_developer_default_can_invoke() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_invoke("bmad/developer"),
            vec!["bmad/developer".to_string()]
        );
    }

    #[test]
    fn acl_pm_default_can_invoke() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_invoke("bmad/pm"),
            vec!["bmad/developer".to_string()]
        );
    }

    #[test]
    fn acl_sm_default_can_invoke() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_invoke("bmad/sm"),
            vec!["bmad/developer".to_string()]
        );
    }

    #[test]
    fn acl_tech_writer_default_can_invoke() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_invoke("bmad/tech-writer"),
            vec!["bmad/developer".to_string()]
        );
    }

    #[test]
    fn acl_ux_designer_default_can_invoke() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_invoke("bmad/ux-designer"),
            vec!["bmad/developer".to_string()]
        );
    }

    #[test]
    fn acl_analyst_default_can_invoke() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        assert_eq!(
            registry.get_can_invoke("bmad/analyst"),
            vec!["bmad/developer".to_string()]
        );
    }

    #[test]
    fn acl_all_agents_can_respond_to_pm_and_sm() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let expected = vec!["bmad/pm".to_string(), "bmad/sm".to_string()];
        let all_agents = [
            "bmad/architect",
            "bmad/qa",
            "bmad/quick-flow-solo-dev",
            "bmad/developer",
            "bmad/pm",
            "bmad/sm",
            "bmad/tech-writer",
            "bmad/ux-designer",
            "bmad/analyst",
        ];
        for agent in &all_agents {
            assert_eq!(
                registry.get_can_respond_to(agent),
                expected,
                "agent {agent} should have can_respond_to = [bmad/pm, bmad/sm]"
            );
        }
    }

    #[test]
    fn acl_unknown_agent_defaults() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let acl = registry.get_acl("bmad/nonexistent");
        assert_eq!(acl.can_invoke, vec!["bmad/developer".to_string()]);
        assert_eq!(
            acl.can_respond_to,
            vec!["bmad/pm".to_string(), "bmad/sm".to_string()]
        );
    }

    #[test]
    fn acl_get_acl_returns_complete_struct() {
        let registry = BmadAgentRegistry::new(&test_manifest_path());
        let acl = registry.get_acl("bmad/architect");
        assert_eq!(
            acl,
            AgentAcl {
                can_invoke: vec![
                    "bmad/analyst".to_string(),
                    "bmad/developer".to_string(),
                    "bmad/ux-designer".to_string(),
                ],
                can_respond_to: vec!["bmad/pm".to_string(), "bmad/sm".to_string()],
            }
        );
    }
}
