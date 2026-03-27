//! Multi-agent session deliberation for workflow steps.
//!
//! A session step brings multiple agents into a shared conversation, with
//! configurable activation strategies and convergence detection.
//!
//! ## Architecture
//!
//! - `SessionConfig` — parsed from the workflow YAML `config.session` block
//! - `execute_session_step()` — runs the turn loop, calling agents via JSON-RPC
//! - `check_convergence()` — evaluates whether the session should end
//! - `build_session_output()` — formats the conversation as a readable transcript

use pulse_plugin_sdk::error::WitPluginError;
use pulse_plugin_sdk::ChatMessage;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::executor::{spawn_plugin_rpc, substitute_templates, StepOutput, StepStatus};
use crate::workspace::WorkspaceConfig;

// ── Story 15-1: Session configuration types ────────────────────────────────

/// Top-level session configuration, deserialized from a workflow step's
/// `config.session` YAML block.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionConfig {
    /// Agents participating in this session.
    pub participants: Vec<SessionParticipant>,
    /// How to decide when the session is done.
    pub convergence: ConvergenceConfig,
    /// Optional system prompt prepended to the conversation.
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Step IDs whose output is injected as initial context.
    #[serde(default)]
    pub context_from: Vec<String>,
}

/// A single participant in a session.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionParticipant {
    /// Agent identifier (e.g. "bmad/architect", "provider-claude-code").
    pub agent: String,
    /// When this participant speaks. Defaults to `EveryTurn`.
    #[serde(default)]
    pub activation: ActivationStrategy,
}

/// Controls when a participant is activated during the turn loop.
///
/// In YAML, use one of:
///   - `every_turn` or `when_mentioned` (plain string)
///   - `on_tag: "TAG"` (map with tag value)
///   - `keyword_match: [kw1, kw2]` (map with keyword list)
#[derive(Debug, Clone, Default)]
pub enum ActivationStrategy {
    /// Speaks every turn (default).
    #[default]
    EveryTurn,
    /// Speaks only when mentioned via `@agent_name` in the last message.
    WhenMentioned,
    /// Speaks when the last message contains a specific tag string.
    OnTag(String),
    /// Speaks when the last message contains any of the given keywords.
    KeywordMatch(Vec<String>),
}

impl<'de> Deserialize<'de> for ActivationStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        match &value {
            serde_yaml::Value::String(s) => match s.as_str() {
                "every_turn" => Ok(ActivationStrategy::EveryTurn),
                "when_mentioned" => Ok(ActivationStrategy::WhenMentioned),
                other => Err(serde::de::Error::custom(format!(
                    "unknown activation strategy: '{other}'"
                ))),
            },
            serde_yaml::Value::Mapping(map) => {
                if let Some(val) = map.get(serde_yaml::Value::String("on_tag".to_string())) {
                    let tag = val
                        .as_str()
                        .ok_or_else(|| serde::de::Error::custom("on_tag value must be a string"))?;
                    Ok(ActivationStrategy::OnTag(tag.to_string()))
                } else if let Some(val) =
                    map.get(serde_yaml::Value::String("keyword_match".to_string()))
                {
                    let seq = val.as_sequence().ok_or_else(|| {
                        serde::de::Error::custom("keyword_match value must be a list")
                    })?;
                    let keywords: Result<Vec<String>, _> = seq
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(|s| s.to_string())
                                .ok_or_else(|| serde::de::Error::custom("keyword must be a string"))
                        })
                        .collect();
                    Ok(ActivationStrategy::KeywordMatch(keywords?))
                } else {
                    Err(serde::de::Error::custom(
                        "expected 'on_tag' or 'keyword_match' key in activation map",
                    ))
                }
            }
            _ => Err(serde::de::Error::custom(
                "activation must be a string or map",
            )),
        }
    }
}

/// Convergence settings controlling when the session ends.
#[derive(Debug, Clone, Deserialize)]
pub struct ConvergenceConfig {
    /// The strategy used to decide convergence.
    pub strategy: ConvergenceStrategy,
    /// Hard cap on the number of turns. Defaults to 4.
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
}

fn default_max_turns() -> u32 {
    4
}

/// Strategy for determining when a session has converged.
///
/// In YAML, use one of:
///   - `fixed_turns` or `unanimous` (plain string)
///   - `stagnation: N` (map with threshold)
#[derive(Debug, Clone)]
pub enum ConvergenceStrategy {
    /// Run exactly `max_turns` turns.
    FixedTurns,
    /// End when all participants signal agreement.
    Unanimous,
    /// End when word overlap exceeds 80% for `threshold` consecutive rounds.
    Stagnation(u32),
}

impl<'de> Deserialize<'de> for ConvergenceStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        match &value {
            serde_yaml::Value::String(s) => match s.as_str() {
                "fixed_turns" => Ok(ConvergenceStrategy::FixedTurns),
                "unanimous" => Ok(ConvergenceStrategy::Unanimous),
                other => Err(serde::de::Error::custom(format!(
                    "unknown convergence strategy: '{other}'"
                ))),
            },
            serde_yaml::Value::Mapping(map) => {
                if let Some(val) = map.get(serde_yaml::Value::String("stagnation".to_string())) {
                    let threshold = val.as_u64().ok_or_else(|| {
                        serde::de::Error::custom("stagnation threshold must be a positive integer")
                    })?;
                    Ok(ConvergenceStrategy::Stagnation(threshold as u32))
                } else {
                    Err(serde::de::Error::custom(
                        "expected 'stagnation' key in convergence strategy map",
                    ))
                }
            }
            _ => Err(serde::de::Error::custom(
                "convergence strategy must be a string or map",
            )),
        }
    }
}

/// Parse a YAML value into a `SessionConfig`.
pub fn parse_session_config(value: &serde_yaml::Value) -> Result<SessionConfig, WitPluginError> {
    serde_yaml::from_value(value.clone())
        .map_err(|e| WitPluginError::invalid_input(format!("invalid session config: {e}")))
}

// ── Story 15-2: Session turn loop with activation evaluation ───────────────

/// Execute a session step: run a multi-agent conversation until convergence.
///
/// Returns `(StepOutput, Option<session_id>)` following the executor convention.
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn execute_session_step(
    config: &SessionConfig,
    step_id: &str,
    initial_context: &str,
    plugins_dir: &Path,
    template_vars: &HashMap<String, String>,
    ws_config: &WorkspaceConfig,
) -> Result<(StepOutput, Option<String>), WitPluginError> {
    let start = std::time::Instant::now();
    let mut conversation: Vec<ChatMessage> = Vec::new();

    // Prepend system prompt if configured
    if let Some(ref sp) = config.system_prompt {
        let resolved = substitute_templates(sp, template_vars);
        conversation.push(ChatMessage::new("system", resolved));
    }

    // Add initial context as a user message
    if !initial_context.is_empty() {
        conversation.push(ChatMessage::user(initial_context.to_string()));
    }

    // Track per-turn responses for convergence checks
    let mut turn_responses: Vec<Vec<(String, String)>> = Vec::new();
    let mut convergence_reason = String::from("max_turns reached");

    // Turn loop
    for turn in 1..=config.convergence.max_turns {
        let mut round_responses: Vec<(String, String)> = Vec::new();

        for participant in &config.participants {
            // Check activation
            if !should_activate(
                &participant.activation,
                &participant.agent,
                &conversation,
                turn,
            ) {
                continue;
            }

            // Call agent
            let response_text = call_agent_in_session(
                &participant.agent,
                &conversation,
                plugins_dir,
                template_vars,
                ws_config,
                step_id,
                turn,
            )?;

            // Record the response
            let mut msg = ChatMessage::assistant(response_text.clone());
            msg.name = Some(participant.agent.clone());
            conversation.push(msg);
            round_responses.push((participant.agent.clone(), response_text));
        }

        turn_responses.push(round_responses);

        // Check convergence
        let (converged, reason) = check_convergence(config, turn, &turn_responses);
        if converged {
            convergence_reason = reason.to_string();
            break;
        }
    }

    let elapsed = start.elapsed().as_millis() as u64;

    // Build human-readable transcript
    let transcript = build_session_output(
        step_id,
        &turn_responses,
        &convergence_reason,
        &config.participants,
    );

    Ok((
        StepOutput {
            step_id: step_id.to_string(),
            status: StepStatus::Success,
            content: Some(transcript),
            execution_time_ms: elapsed,
            error: None,
        },
        None,
    ))
}

/// Determine whether a participant should be activated on the current turn.
///
/// On turn 1, ALL participants are activated regardless of strategy.
pub fn should_activate(
    strategy: &ActivationStrategy,
    agent_name: &str,
    conversation: &[ChatMessage],
    turn: u32,
) -> bool {
    // Turn 1: everyone speaks
    if turn == 1 {
        return true;
    }

    match strategy {
        ActivationStrategy::EveryTurn => true,
        ActivationStrategy::WhenMentioned => {
            let last_content = last_message_content(conversation);
            let mention = format!("@{}", agent_name);
            last_content
                .to_lowercase()
                .contains(&mention.to_lowercase())
        }
        ActivationStrategy::OnTag(tag) => {
            let last_content = last_message_content(conversation);
            last_content.contains(tag.as_str())
        }
        ActivationStrategy::KeywordMatch(keywords) => {
            let last_content = last_message_content(conversation);
            let lower = last_content.to_lowercase();
            keywords.iter().any(|kw| lower.contains(&kw.to_lowercase()))
        }
    }
}

/// Get the content of the last message in the conversation, or empty string.
fn last_message_content(conversation: &[ChatMessage]) -> &str {
    conversation
        .last()
        .map(|m| m.content.as_str())
        .unwrap_or("")
}

/// Call an agent via JSON-RPC, passing the full conversation history.
#[allow(dead_code)]
fn call_agent_in_session(
    agent: &str,
    conversation: &[ChatMessage],
    plugins_dir: &Path,
    template_vars: &HashMap<String, String>,
    ws_config: &WorkspaceConfig,
    step_id: &str,
    turn: u32,
) -> Result<String, WitPluginError> {
    let claude_binary = plugins_dir.join("provider-claude-code");
    if !claude_binary.exists() {
        return Err(WitPluginError::not_found(format!(
            "session step '{}': provider-claude-code not found at {}",
            step_id,
            claude_binary.display()
        )));
    }

    // Serialize conversation history as JSON for the prompt
    let history_json = serde_json::to_string(conversation)
        .map_err(|e| WitPluginError::internal(format!("failed to serialize conversation: {e}")))?;

    let prompt = format!(
        "You are agent '{}' in a multi-agent session (turn {}).\n\n\
         ## Conversation History\n```json\n{}\n```\n\n\
         Respond as your agent role. Be concise and constructive.",
        agent, turn, history_json
    );

    let mut parameters = serde_json::Map::new();

    // Use the default model from workspace config if available
    if let Some(ref model) = ws_config.defaults.default_model {
        parameters.insert("model_tier".to_string(), serde_json::json!(model));
    }

    // Forward session_id if available
    if let Some(session_id) = template_vars.get("session_id") {
        parameters.insert("session_id".to_string(), serde_json::json!(session_id));
    }

    // Forward working_dir
    if let Some(wd) = template_vars.get("working_dir") {
        parameters.insert("working_dir".to_string(), serde_json::json!(wd));
    }

    let rpc_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "step-executor.execute",
        "params": {
            "task": {
                "task_id": format!("session-{}-turn{}-{}", step_id, turn, agent),
                "description": prompt,
                "input": {
                    "prompt": prompt,
                },
            },
            "config": {
                "step_id": format!("{}-turn{}-{}", step_id, turn, agent),
                "step_type": "agent",
                "parameters": parameters,
            }
        }
    });

    let response = spawn_plugin_rpc(&claude_binary, &rpc_request, 120)?;

    // Extract response text
    extract_session_response(&response)
}

/// Extract the text content from a JSON-RPC response.
#[allow(dead_code)]
fn extract_session_response(response: &serde_json::Value) -> Result<String, WitPluginError> {
    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(WitPluginError::internal(format!(
            "session agent error: {msg}"
        )));
    }

    let content_str = response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");

    // Try to parse inner JSON (provider-claude-code wraps result)
    if let Ok(inner) = serde_json::from_str::<serde_json::Value>(content_str) {
        if let Some(text) = inner.get("result").and_then(|r| r.as_str()) {
            return Ok(text.to_string());
        }
    }

    Ok(content_str.to_string())
}

// ── Story 15-3: Convergence evaluation and session output ──────────────────

/// Agreement signal phrases (case-insensitive substring match).
const AGREEMENT_SIGNALS: &[&str] = &[
    "i agree",
    "approved",
    "no objections",
    "consensus reached",
    "lgtm",
    "looks good",
];

/// Check whether the session has converged.
///
/// Returns `(converged, reason)`.
pub fn check_convergence<'a>(
    config: &'a SessionConfig,
    turn: u32,
    turn_responses: &[Vec<(String, String)>],
) -> (bool, &'a str) {
    // Always stop at max_turns
    if turn >= config.convergence.max_turns {
        return (true, "fixed_turns reached");
    }

    match &config.convergence.strategy {
        ConvergenceStrategy::FixedTurns => {
            // Only converge when we hit max_turns (handled above)
            (false, "")
        }
        ConvergenceStrategy::Unanimous => {
            // Check if all participants' last responses contain agreement signals
            if let Some(last_round) = turn_responses.last() {
                let all_agree = config.participants.iter().all(|p| {
                    last_round
                        .iter()
                        .filter(|(agent, _)| agent == &p.agent)
                        .any(|(_, text)| {
                            let lower = text.to_lowercase();
                            AGREEMENT_SIGNALS
                                .iter()
                                .any(|signal| lower.contains(signal))
                        })
                });
                if all_agree && !last_round.is_empty() {
                    return (true, "unanimous agreement");
                }
            }
            (false, "")
        }
        ConvergenceStrategy::Stagnation(threshold) => {
            let threshold = *threshold as usize;
            if turn_responses.len() < 2 || threshold == 0 {
                return (false, "");
            }

            // Check if the last `threshold` consecutive round transitions have >80% word overlap
            let consecutive_stagnant = count_stagnant_rounds(turn_responses);
            if consecutive_stagnant >= threshold {
                return (true, "stagnation detected");
            }
            (false, "")
        }
    }
}

/// Count consecutive stagnant round transitions from the end of the responses.
///
/// A transition from round N to round N+1 is "stagnant" if >80% of words overlap.
fn count_stagnant_rounds(turn_responses: &[Vec<(String, String)>]) -> usize {
    if turn_responses.len() < 2 {
        return 0;
    }

    let mut count = 0;
    // Walk backwards through consecutive round pairs
    for i in (1..turn_responses.len()).rev() {
        let prev_words = collect_round_words(&turn_responses[i - 1]);
        let curr_words = collect_round_words(&turn_responses[i]);

        if prev_words.is_empty() && curr_words.is_empty() {
            count += 1;
            continue;
        }

        let overlap = word_overlap_ratio(&prev_words, &curr_words);
        if overlap > 0.8 {
            count += 1;
        } else {
            break;
        }
    }
    count
}

/// Collect all words from a round of responses as a set (lowercased).
fn collect_round_words(round: &[(String, String)]) -> HashSet<String> {
    round
        .iter()
        .flat_map(|(_, text)| text.split_whitespace().map(|w| w.to_lowercase()))
        .collect()
}

/// Compute word overlap ratio between two word sets.
///
/// Returns |intersection| / |union|, or 0.0 if both sets are empty.
fn word_overlap_ratio(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        return 0.0;
    }
    intersection / union
}

/// Build a human-readable transcript of the session.
pub fn build_session_output(
    step_id: &str,
    turn_responses: &[Vec<(String, String)>],
    convergence_reason: &str,
    participants: &[SessionParticipant],
) -> String {
    let participant_names: Vec<&str> = participants.iter().map(|p| p.agent.as_str()).collect();

    let mut output = String::new();
    output.push_str(&format!("## Session: {}\n", step_id));
    output.push_str(&format!("Turns: {}\n", turn_responses.len()));
    output.push_str(&format!("Convergence: {}\n", convergence_reason));
    output.push_str(&format!("Participants: {}\n", participant_names.join(", ")));

    for (i, round) in turn_responses.iter().enumerate() {
        output.push_str(&format!("\n### Turn {}\n", i + 1));
        for (agent, text) in round {
            output.push_str(&format!("**{}:** {}\n", agent, text));
        }
    }

    output
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Story 15-1: Deserialization tests ──────────────────────────────

    #[test]
    fn parse_full_session_config() {
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
participants:
  - agent: bmad/architect
    activation: every_turn
  - agent: bmad/qa
    activation: when_mentioned
convergence:
  strategy: fixed_turns
  max_turns: 3
system_prompt: "You are in a design session"
context_from:
  - step-1
  - step-2
"#,
        )
        .unwrap();

        let config = parse_session_config(&yaml).unwrap();
        assert_eq!(config.participants.len(), 2);
        assert_eq!(config.participants[0].agent, "bmad/architect");
        assert_eq!(config.convergence.max_turns, 3);
        assert_eq!(
            config.system_prompt.as_deref(),
            Some("You are in a design session")
        );
        assert_eq!(config.context_from.len(), 2);
    }

    #[test]
    fn parse_default_activation_strategy() {
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
participants:
  - agent: bmad/dev
convergence:
  strategy: fixed_turns
"#,
        )
        .unwrap();

        let config = parse_session_config(&yaml).unwrap();
        assert!(matches!(
            config.participants[0].activation,
            ActivationStrategy::EveryTurn
        ));
    }

    #[test]
    fn parse_default_max_turns() {
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
participants:
  - agent: bmad/dev
convergence:
  strategy: fixed_turns
"#,
        )
        .unwrap();

        let config = parse_session_config(&yaml).unwrap();
        assert_eq!(config.convergence.max_turns, 4);
    }

    #[test]
    fn parse_on_tag_activation() {
        let yaml_str = "
participants:
  - agent: bmad/security
    activation:
      on_tag: SECURITY_REVIEW
convergence:
  strategy: fixed_turns
";
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(yaml_str).unwrap();

        let config = parse_session_config(&yaml).unwrap();
        match &config.participants[0].activation {
            ActivationStrategy::OnTag(tag) => assert_eq!(tag, "SECURITY_REVIEW"),
            other => panic!("expected OnTag, got {:?}", other),
        }
    }

    #[test]
    fn parse_keyword_match_activation() {
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
participants:
  - agent: bmad/qa
    activation:
      keyword_match:
        - testing
        - quality
convergence:
  strategy: fixed_turns
"#,
        )
        .unwrap();

        let config = parse_session_config(&yaml).unwrap();
        match &config.participants[0].activation {
            ActivationStrategy::KeywordMatch(kws) => {
                assert_eq!(kws, &["testing", "quality"]);
            }
            other => panic!("expected KeywordMatch, got {:?}", other),
        }
    }

    #[test]
    fn parse_unanimous_convergence() {
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
participants:
  - agent: bmad/dev
convergence:
  strategy: unanimous
  max_turns: 6
"#,
        )
        .unwrap();

        let config = parse_session_config(&yaml).unwrap();
        assert!(matches!(
            config.convergence.strategy,
            ConvergenceStrategy::Unanimous
        ));
        assert_eq!(config.convergence.max_turns, 6);
    }

    #[test]
    fn parse_stagnation_convergence() {
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
participants:
  - agent: bmad/dev
convergence:
  strategy:
    stagnation: 2
"#,
        )
        .unwrap();

        let config = parse_session_config(&yaml).unwrap();
        match &config.convergence.strategy {
            ConvergenceStrategy::Stagnation(threshold) => assert_eq!(*threshold, 2),
            other => panic!("expected Stagnation, got {:?}", other),
        }
    }

    #[test]
    fn parse_missing_participants_fails() {
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
convergence:
  strategy: fixed_turns
"#,
        )
        .unwrap();

        let err = parse_session_config(&yaml).unwrap_err();
        assert_eq!(err.code, "invalid_input");
    }

    #[test]
    fn parse_default_system_prompt_is_none() {
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
participants:
  - agent: bmad/dev
convergence:
  strategy: fixed_turns
"#,
        )
        .unwrap();

        let config = parse_session_config(&yaml).unwrap();
        assert!(config.system_prompt.is_none());
        assert!(config.context_from.is_empty());
    }

    // ── Story 15-2: Activation tests ───────────────────────────────────

    #[test]
    fn should_activate_every_turn_always_true() {
        let conv = vec![ChatMessage::user("hello")];
        assert!(should_activate(
            &ActivationStrategy::EveryTurn,
            "agent1",
            &conv,
            2
        ));
    }

    #[test]
    fn should_activate_turn_1_always_true() {
        let conv = vec![];
        // Even WhenMentioned should activate on turn 1
        assert!(should_activate(
            &ActivationStrategy::WhenMentioned,
            "agent1",
            &conv,
            1
        ));
        assert!(should_activate(
            &ActivationStrategy::OnTag("tag".to_string()),
            "agent1",
            &conv,
            1
        ));
        assert!(should_activate(
            &ActivationStrategy::KeywordMatch(vec!["kw".to_string()]),
            "agent1",
            &conv,
            1
        ));
    }

    #[test]
    fn should_activate_when_mentioned_positive() {
        let conv = vec![ChatMessage::assistant("I think @agent1 should weigh in")];
        assert!(should_activate(
            &ActivationStrategy::WhenMentioned,
            "agent1",
            &conv,
            2
        ));
    }

    #[test]
    fn should_activate_when_mentioned_case_insensitive() {
        let conv = vec![ChatMessage::assistant("Let's hear from @AGENT1")];
        assert!(should_activate(
            &ActivationStrategy::WhenMentioned,
            "agent1",
            &conv,
            2
        ));
    }

    #[test]
    fn should_activate_when_mentioned_negative() {
        let conv = vec![ChatMessage::assistant("I think agent2 should respond")];
        assert!(!should_activate(
            &ActivationStrategy::WhenMentioned,
            "agent1",
            &conv,
            2
        ));
    }

    #[test]
    fn should_activate_on_tag_positive() {
        let conv = vec![ChatMessage::assistant(
            "This needs #security-review attention",
        )];
        assert!(should_activate(
            &ActivationStrategy::OnTag("#security-review".to_string()),
            "agent1",
            &conv,
            2
        ));
    }

    #[test]
    fn should_activate_on_tag_negative() {
        let conv = vec![ChatMessage::assistant("Regular message")];
        assert!(!should_activate(
            &ActivationStrategy::OnTag("#security-review".to_string()),
            "agent1",
            &conv,
            2
        ));
    }

    #[test]
    fn should_activate_keyword_match_positive() {
        let conv = vec![ChatMessage::assistant(
            "We need to focus on Testing this component",
        )];
        assert!(should_activate(
            &ActivationStrategy::KeywordMatch(vec!["testing".to_string(), "quality".to_string()]),
            "agent1",
            &conv,
            2
        ));
    }

    #[test]
    fn should_activate_keyword_match_negative() {
        let conv = vec![ChatMessage::assistant("Let's discuss architecture")];
        assert!(!should_activate(
            &ActivationStrategy::KeywordMatch(vec!["testing".to_string(), "quality".to_string()]),
            "agent1",
            &conv,
            2
        ));
    }

    #[test]
    fn should_activate_empty_conversation() {
        let conv: Vec<ChatMessage> = vec![];
        // Turn > 1, empty conversation, WhenMentioned -> false
        assert!(!should_activate(
            &ActivationStrategy::WhenMentioned,
            "agent1",
            &conv,
            2
        ));
    }

    // ── Story 15-3: Convergence tests ──────────────────────────────────

    fn make_config(strategy: ConvergenceStrategy, max_turns: u32) -> SessionConfig {
        SessionConfig {
            participants: vec![
                SessionParticipant {
                    agent: "agent1".to_string(),
                    activation: ActivationStrategy::EveryTurn,
                },
                SessionParticipant {
                    agent: "agent2".to_string(),
                    activation: ActivationStrategy::EveryTurn,
                },
            ],
            convergence: ConvergenceConfig {
                strategy,
                max_turns,
            },
            system_prompt: None,
            context_from: vec![],
        }
    }

    #[test]
    fn convergence_fixed_turns_at_max() {
        let config = make_config(ConvergenceStrategy::FixedTurns, 3);
        let responses = vec![
            vec![("agent1".to_string(), "hello".to_string())],
            vec![("agent1".to_string(), "world".to_string())],
            vec![("agent1".to_string(), "done".to_string())],
        ];
        let (converged, reason) = check_convergence(&config, 3, &responses);
        assert!(converged);
        assert_eq!(reason, "fixed_turns reached");
    }

    #[test]
    fn convergence_fixed_turns_before_max() {
        let config = make_config(ConvergenceStrategy::FixedTurns, 5);
        let responses = vec![vec![("agent1".to_string(), "hello".to_string())]];
        let (converged, _) = check_convergence(&config, 1, &responses);
        assert!(!converged);
    }

    #[test]
    fn convergence_unanimous_all_agree() {
        let config = make_config(ConvergenceStrategy::Unanimous, 10);
        let responses = vec![vec![
            (
                "agent1".to_string(),
                "I agree with the proposal".to_string(),
            ),
            (
                "agent2".to_string(),
                "LGTM, no objections from me".to_string(),
            ),
        ]];
        let (converged, reason) = check_convergence(&config, 1, &responses);
        assert!(converged);
        assert_eq!(reason, "unanimous agreement");
    }

    #[test]
    fn convergence_unanimous_partial_agreement() {
        let config = make_config(ConvergenceStrategy::Unanimous, 10);
        let responses = vec![vec![
            (
                "agent1".to_string(),
                "I agree with the approach".to_string(),
            ),
            (
                "agent2".to_string(),
                "I have concerns about this design".to_string(),
            ),
        ]];
        let (converged, _) = check_convergence(&config, 1, &responses);
        assert!(!converged);
    }

    #[test]
    fn convergence_unanimous_approved_signal() {
        let config = make_config(ConvergenceStrategy::Unanimous, 10);
        let responses = vec![vec![
            ("agent1".to_string(), "Approved".to_string()),
            ("agent2".to_string(), "Looks good to me".to_string()),
        ]];
        let (converged, reason) = check_convergence(&config, 1, &responses);
        assert!(converged);
        assert_eq!(reason, "unanimous agreement");
    }

    #[test]
    fn convergence_unanimous_consensus_reached() {
        let config = make_config(ConvergenceStrategy::Unanimous, 10);
        let responses = vec![vec![
            (
                "agent1".to_string(),
                "Consensus reached on the approach".to_string(),
            ),
            (
                "agent2".to_string(),
                "I think we have consensus reached".to_string(),
            ),
        ]];
        let (converged, reason) = check_convergence(&config, 1, &responses);
        assert!(converged);
        assert_eq!(reason, "unanimous agreement");
    }

    #[test]
    fn convergence_stagnation_detected() {
        let config = make_config(ConvergenceStrategy::Stagnation(2), 10);
        // Three rounds with very similar content
        let responses = vec![
            vec![
                (
                    "agent1".to_string(),
                    "the quick brown fox jumps".to_string(),
                ),
                ("agent2".to_string(), "over the lazy dog today".to_string()),
            ],
            vec![
                (
                    "agent1".to_string(),
                    "the quick brown fox jumps".to_string(),
                ),
                ("agent2".to_string(), "over the lazy dog today".to_string()),
            ],
            vec![
                (
                    "agent1".to_string(),
                    "the quick brown fox jumps".to_string(),
                ),
                ("agent2".to_string(), "over the lazy dog today".to_string()),
            ],
        ];
        let (converged, reason) = check_convergence(&config, 3, &responses);
        assert!(converged);
        assert_eq!(reason, "stagnation detected");
    }

    #[test]
    fn convergence_stagnation_not_enough_rounds() {
        let config = make_config(ConvergenceStrategy::Stagnation(3), 10);
        // Only 2 rounds
        let responses = vec![
            vec![("agent1".to_string(), "hello world".to_string())],
            vec![("agent1".to_string(), "hello world".to_string())],
        ];
        // Only 1 transition (rounds 1->2), need threshold of 3
        let (converged, _) = check_convergence(&config, 2, &responses);
        assert!(!converged);
    }

    #[test]
    fn convergence_stagnation_different_content_resets() {
        let config = make_config(ConvergenceStrategy::Stagnation(2), 10);
        let responses = vec![
            vec![("agent1".to_string(), "topic A discussion".to_string())],
            vec![(
                "agent1".to_string(),
                "completely different topic B".to_string(),
            )],
            vec![(
                "agent1".to_string(),
                "completely different topic B".to_string(),
            )],
            vec![(
                "agent1".to_string(),
                "completely different topic B".to_string(),
            )],
        ];
        // Rounds 2->3 and 3->4 are stagnant = 2 consecutive transitions
        let (converged, reason) = check_convergence(&config, 4, &responses);
        assert!(converged);
        assert_eq!(reason, "stagnation detected");
    }

    #[test]
    fn convergence_max_turns_overrides_strategy() {
        // Even for Unanimous, max_turns is a hard cap
        let config = make_config(ConvergenceStrategy::Unanimous, 3);
        let responses = vec![
            vec![("agent1".to_string(), "no agreement yet".to_string())],
            vec![("agent1".to_string(), "still discussing".to_string())],
            vec![("agent1".to_string(), "another round".to_string())],
        ];
        let (converged, reason) = check_convergence(&config, 3, &responses);
        assert!(converged);
        assert_eq!(reason, "fixed_turns reached");
    }

    // ── Story 15-3: Word overlap tests ─────────────────────────────────

    #[test]
    fn word_overlap_identical_sets() {
        let a: HashSet<String> = ["hello", "world"].iter().map(|s| s.to_string()).collect();
        let b = a.clone();
        assert!((word_overlap_ratio(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn word_overlap_disjoint_sets() {
        let a: HashSet<String> = ["hello", "world"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["foo", "bar"].iter().map(|s| s.to_string()).collect();
        assert!((word_overlap_ratio(&a, &b)).abs() < f64::EPSILON);
    }

    #[test]
    fn word_overlap_partial() {
        let a: HashSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["b", "c", "d"].iter().map(|s| s.to_string()).collect();
        // intersection = {b, c} = 2, union = {a, b, c, d} = 4
        assert!((word_overlap_ratio(&a, &b) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn word_overlap_both_empty() {
        let a: HashSet<String> = HashSet::new();
        let b: HashSet<String> = HashSet::new();
        assert!((word_overlap_ratio(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    // ── Story 15-3: Output builder tests ───────────────────────────────

    #[test]
    fn build_session_output_basic() {
        let participants = vec![
            SessionParticipant {
                agent: "architect".to_string(),
                activation: ActivationStrategy::EveryTurn,
            },
            SessionParticipant {
                agent: "qa".to_string(),
                activation: ActivationStrategy::EveryTurn,
            },
        ];

        let turn_responses = vec![
            vec![
                ("architect".to_string(), "I propose pattern X".to_string()),
                ("qa".to_string(), "What about edge cases?".to_string()),
            ],
            vec![
                (
                    "architect".to_string(),
                    "Good point, handling them now".to_string(),
                ),
                ("qa".to_string(), "LGTM".to_string()),
            ],
        ];

        let output = build_session_output(
            "review-session",
            &turn_responses,
            "unanimous agreement",
            &participants,
        );

        assert!(output.contains("## Session: review-session"));
        assert!(output.contains("Turns: 2"));
        assert!(output.contains("Convergence: unanimous agreement"));
        assert!(output.contains("Participants: architect, qa"));
        assert!(output.contains("### Turn 1"));
        assert!(output.contains("### Turn 2"));
        assert!(output.contains("**architect:** I propose pattern X"));
        assert!(output.contains("**qa:** What about edge cases?"));
        assert!(output.contains("**qa:** LGTM"));
    }

    #[test]
    fn build_session_output_empty_turns() {
        let participants = vec![SessionParticipant {
            agent: "agent1".to_string(),
            activation: ActivationStrategy::EveryTurn,
        }];

        let turn_responses: Vec<Vec<(String, String)>> = vec![];

        let output = build_session_output(
            "empty-session",
            &turn_responses,
            "max_turns reached",
            &participants,
        );

        assert!(output.contains("Turns: 0"));
        assert!(!output.contains("### Turn"));
    }

    #[test]
    fn build_session_output_single_turn() {
        let participants = vec![SessionParticipant {
            agent: "solo".to_string(),
            activation: ActivationStrategy::EveryTurn,
        }];

        let turn_responses = vec![vec![("solo".to_string(), "Only one response".to_string())]];

        let output = build_session_output(
            "solo-session",
            &turn_responses,
            "fixed_turns reached",
            &participants,
        );

        assert!(output.contains("Turns: 1"));
        assert!(output.contains("### Turn 1"));
        assert!(output.contains("**solo:** Only one response"));
    }

    // ── Stagnation edge cases ──────────────────────────────────────────

    #[test]
    fn stagnation_threshold_zero_never_converges() {
        let config = make_config(ConvergenceStrategy::Stagnation(0), 10);
        let responses = vec![
            vec![("agent1".to_string(), "same".to_string())],
            vec![("agent1".to_string(), "same".to_string())],
        ];
        let (converged, _) = check_convergence(&config, 2, &responses);
        assert!(!converged);
    }

    #[test]
    fn stagnation_single_round_never_stagnant() {
        let config = make_config(ConvergenceStrategy::Stagnation(1), 10);
        let responses = vec![vec![("agent1".to_string(), "hello".to_string())]];
        let (converged, _) = check_convergence(&config, 1, &responses);
        assert!(!converged);
    }

    #[test]
    fn unanimous_empty_round_does_not_converge() {
        let config = make_config(ConvergenceStrategy::Unanimous, 10);
        let responses: Vec<Vec<(String, String)>> = vec![vec![]];
        let (converged, _) = check_convergence(&config, 1, &responses);
        assert!(!converged);
    }

    #[test]
    fn unanimous_missing_participant_does_not_converge() {
        let config = make_config(ConvergenceStrategy::Unanimous, 10);
        // Only agent1 responds, agent2 is missing
        let responses = vec![vec![(
            "agent1".to_string(),
            "I agree completely".to_string(),
        )]];
        let (converged, _) = check_convergence(&config, 1, &responses);
        assert!(!converged);
    }
}
