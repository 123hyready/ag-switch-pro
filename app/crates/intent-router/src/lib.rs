//! Stateful intent routing independent of desktop, database and HTTP transport.
//! Each conversation branch serializes decisions; different branches run independently.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

const MAX_SESSIONS: usize = 1024;
const SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const DECISION_TTL: Duration = Duration::from_secs(10 * 60);
const FAILURE_COOLDOWN: Duration = Duration::from_secs(30);
const MAX_RECENT_REQUESTS: usize = 64;
const GOAL_CHARS: usize = 2400;
const OBSERVATION_CHARS: usize = 1600;
// Routing uses a coarse estimate, not provider tokenization. Image URL/base64
// length is transport size; reserve visual tokens independently of that size.
const ESTIMATED_IMAGE_TOKENS: u64 = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelTarget {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Worker {
    pub id: String,
    pub provider_id: String,
    pub model: String,
    pub description: String,
    pub context_tokens: u64,
    pub tools: bool,
    pub images: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    Stage,
    Turn,
    Observe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteGroup {
    pub id: String,
    pub name: String,
    pub app_type: String,
    pub enabled: bool,
    pub mode: RouteMode,
    pub judge: ModelTarget,
    pub workers: Vec<Worker>,
    pub default_worker: String,
    pub fallback_worker: String,
    pub pinned_worker: Option<String>,
    pub max_hold_requests: u32,
    pub max_judge_calls: u32,
    pub judge_timeout_ms: u64,
    pub revision: u64,
}

impl RouteGroup {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 80
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(
                "Route group ID must contain 1–80 letters, digits, hyphens or underscores".into(),
            );
        }
        if self.name.trim().is_empty() || self.name.len() > 240 {
            return Err("Invalid route group name".into());
        }
        if !matches!(self.app_type.as_str(), "claude" | "codex") {
            return Err("Only Claude Code and Codex are supported".into());
        }
        if self.judge.provider_id.is_empty() || self.judge.model.trim().is_empty() {
            return Err("A judge model is required".into());
        }
        if self.workers.is_empty() || self.workers.len() > 32 {
            return Err("Configure 1–32 worker models".into());
        }
        let mut ids = std::collections::HashSet::new();
        for worker in &self.workers {
            if worker.id.is_empty() || !ids.insert(&worker.id) {
                return Err("Worker IDs must be unique and nonempty".into());
            }
            if worker.provider_id.is_empty()
                || worker.model.trim().is_empty()
                || worker.model.len() > 200
                || worker.description.len() > 2000
            {
                return Err("Invalid worker model or description".into());
            }
            if worker.context_tokens < 1024 {
                return Err("Worker context must be at least 1024 tokens".into());
            }
        }
        if !ids.contains(&self.default_worker) || !ids.contains(&self.fallback_worker) {
            return Err("Default and fallback must name configured workers".into());
        }
        if self
            .pinned_worker
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            return Err("Pinned worker is not configured".into());
        }
        if !(1..=20).contains(&self.max_hold_requests)
            || self.max_judge_calls > 1000
            || !(100..=30_000).contains(&self.judge_timeout_ms)
        {
            return Err("Invalid hold limit, judge budget or timeout".into());
        }
        Ok(())
    }

    pub fn virtual_model(&self) -> String {
        if self.app_type == "claude" {
            format!("ags-claude-group-{}", self.id)
        } else {
            format!("ags-group/{}", self.id)
        }
    }

    pub fn worker(&self, id: &str) -> Option<&Worker> {
        self.workers.iter().find(|worker| worker.id == id)
    }
}

#[derive(Debug, Clone)]
pub struct RequestFacts {
    pub app_type: String,
    pub session: Option<String>,
    pub branch: String,
    pub fingerprint: String,
    pub goal: String,
    pub observation: String,
    pub phase_signal: Option<String>,
    pub tool_failed: bool,
    pub sealed_state: bool,
    pub needs_tools: bool,
    pub needs_images: bool,
    pub estimated_tokens: u64,
    pub unavailable_workers: Vec<String>,
}

fn bounded(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit * 3 / 4).collect();
    let tail: String = text.chars().skip(count - limit / 4).collect();
    format!("{head}\n[truncated]\n{tail}")
}

fn text_content(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.into();
    }
    if let Some(parts) = value.as_array() {
        return parts
            .iter()
            .filter_map(|part| {
                let kind = part.get("type").and_then(Value::as_str).unwrap_or("");
                if matches!(kind, "tool_result" | "tool_use" | "input_image" | "image") {
                    return None;
                }
                part.get("text").and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    String::new()
}

fn contains_type(value: &Value, types: &[&str]) -> bool {
    match value {
        Value::Object(object) => {
            object
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| types.contains(&kind))
                || object.values().any(|child| contains_type(child, types))
        }
        Value::Array(items) => items.iter().any(|child| contains_type(child, types)),
        _ => false,
    }
}

/// Only message content and structured tool results contain image inputs.
/// Tool schemas, arguments and metadata may describe images without sending one.
fn collect_input_images<'a>(value: &'a Value, images: &mut Vec<&'a Value>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_input_images(item, images);
            }
        }
        Value::Object(object) => {
            if object
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| matches!(kind, "image" | "image_url" | "input_image"))
            {
                images.push(value);
            } else {
                for key in ["content", "output"] {
                    if let Some(content) = object.get(key) {
                        collect_input_images(content, images);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Inspect protocol items, not arbitrary JSON such as tool schemas, arguments,
/// metadata or tool results that happen to contain a `signature` property.
fn contains_sealed(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
            if object.get("role").and_then(Value::as_str) == Some("tool")
                || matches!(
                    kind,
                    "tool_use" | "tool_result" | "function_call" | "function_call_output"
                )
            {
                return false;
            }
            matches!(kind, "compaction" | "agent_message" | "redacted_thinking")
                || (kind == "reasoning"
                    && object
                        .get("encrypted_content")
                        .is_some_and(|value| !value.is_null()))
                || (kind == "thinking"
                    && object
                        .get("signature")
                        .is_some_and(|value| !value.is_null()))
                || object
                    .get("thoughtSignature")
                    .is_some_and(|value| !value.is_null())
                || ["content", "parts"]
                    .iter()
                    .any(|key| object.get(*key).is_some_and(contains_sealed))
        }
        Value::Array(items) => items.iter().any(contains_sealed),
        _ => false,
    }
}

fn tool_phase(name: &str, arguments: &Value) -> Option<String> {
    let name = name.to_lowercase();
    let arguments = arguments.to_string().to_lowercase();
    let phase =
        if name.contains("patch") || name.contains("write_file") || name.contains("edit_file") {
            "implement"
        } else if name.contains("test")
            || ((name.contains("exec") || name.contains("shell") || name.contains("command"))
                && [
                    " test",
                    "pytest",
                    "vitest",
                    "cargo test",
                    "tsc ",
                    "typecheck",
                ]
                .iter()
                .any(|word| arguments.contains(word)))
        {
            "verify"
        } else if ["read", "search", "find", "list", "view"]
            .iter()
            .any(|word| name.contains(word))
        {
            "collect"
        } else {
            return None;
        };
    Some(phase.into())
}

impl RequestFacts {
    pub fn from_body(app_type: &str, session: Option<&str>, branch: &str, body: &Value) -> Self {
        let encoded = body.to_string();
        let fingerprint = format!("{:x}", Sha256::digest(encoded.as_bytes()));
        let mut goal = body
            .get("input")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut observation = String::new();
        let mut phase_signal = None;
        let mut tool_failed = false;
        if let Some(messages) = body
            .get("messages")
            .or_else(|| body.get("input"))
            .and_then(Value::as_array)
        {
            for message in messages {
                let kind = message.get("type").and_then(Value::as_str).unwrap_or("");
                let role = message.get("role").and_then(Value::as_str).unwrap_or("");
                if role == "user" {
                    let text = text_content(&message["content"]);
                    if !text.trim().is_empty() {
                        goal = text;
                    }
                }
                if role == "tool" || kind == "function_call_output" {
                    observation = message
                        .get("output")
                        .or_else(|| message.get("content"))
                        .map(|content| {
                            content
                                .as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| content.to_string())
                        })
                        .unwrap_or_default();
                    tool_failed = message
                        .get("is_error")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                }
                if kind == "function_call" {
                    phase_signal = tool_phase(
                        message["name"].as_str().unwrap_or(""),
                        &message["arguments"],
                    );
                }
                if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        phase_signal = tool_phase(
                            call["function"]["name"].as_str().unwrap_or(""),
                            &call["function"]["arguments"],
                        );
                    }
                }
                if let Some(parts) = message.get("content").and_then(Value::as_array) {
                    for part in parts {
                        match part["type"].as_str().unwrap_or("") {
                            "tool_use" => {
                                phase_signal =
                                    tool_phase(part["name"].as_str().unwrap_or(""), &part["input"]);
                            }
                            "tool_result" => {
                                observation = part["content"]
                                    .as_str()
                                    .map(str::to_string)
                                    .unwrap_or_else(|| part["content"].to_string());
                                tool_failed = part["is_error"].as_bool().unwrap_or(false);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        let observation_lower = observation.to_lowercase();
        tool_failed |= observation_lower.contains("error:")
            || observation_lower.contains("test failed")
            || observation_lower.contains("exit code: 1")
            || observation_lower.contains("exit code: 2");
        if tool_failed {
            phase_signal = Some("diagnose".into());
        }
        let output_reserve = body
            .get("max_tokens")
            .or_else(|| body.get("max_output_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(4096);
        let mut images = Vec::new();
        for key in ["input", "messages"] {
            if let Some(content) = body.get(key) {
                collect_input_images(content, &mut images);
            }
        }
        let image_bytes: usize = images.iter().map(|image| image.to_string().len()).sum();
        let estimated_tokens = (encoded.len().saturating_sub(image_bytes) as u64)
            .div_ceil(2)
            .saturating_add(ESTIMATED_IMAGE_TOKENS.saturating_mul(images.len() as u64))
            .saturating_add(output_reserve);
        Self {
            app_type: app_type.into(),
            session: session.filter(|s| !s.trim().is_empty()).map(str::to_string),
            branch: branch.into(),
            fingerprint,
            goal: bounded(&goal, GOAL_CHARS),
            observation: bounded(&observation, OBSERVATION_CHARS),
            phase_signal,
            tool_failed,
            sealed_state: body
                .get("previous_response_id")
                .is_some_and(|id| !id.is_null())
                || ["input", "messages"]
                    .iter()
                    .any(|key| body.get(*key).is_some_and(contains_sealed)),
            needs_tools: body
                .get("tools")
                .and_then(Value::as_array)
                .is_some_and(|tools| !tools.is_empty())
                || contains_type(
                    body,
                    &[
                        "tool_use",
                        "tool_result",
                        "function_call",
                        "function_call_output",
                    ],
                ),
            needs_images: !images.is_empty(),
            estimated_tokens,
            unavailable_workers: Vec::new(),
        }
    }

    pub fn accepts(&self, worker: &Worker) -> bool {
        (!self.needs_tools || worker.tools)
            && (!self.needs_images || worker.images)
            && self.estimated_tokens <= worker.context_tokens
            && !self.unavailable_workers.contains(&worker.id)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionInput {
    pub goal: String,
    pub observation: String,
    pub previous_phase: Option<String>,
    pub previous_worker: Option<String>,
    pub phase_signal: Option<String>,
    pub candidates: Vec<Worker>,
    pub max_hold_requests: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JudgeAnswer {
    pub worker_id: String,
    pub phase: String,
    pub hold_requests: u32,
    pub reason: String,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(skip)]
    pub cost_usd: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionSource {
    Judge,
    Hold,
    Duplicate,
    Fallback,
    Budget,
    Pinned,
    Continuity,
    Observe,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub worker: Worker,
    pub recommended_worker: String,
    pub phase: String,
    pub reason: String,
    pub source: DecisionSource,
    pub judge_millis: u64,
    pub judge_input_tokens: Option<u64>,
    pub judge_output_tokens: Option<u64>,
    pub judge_cost_usd: Option<String>,
}

struct Conversation {
    selected: Option<Selection>,
    accepted_worker: Option<Worker>,
    goal: String,
    signal: Option<String>,
    remaining: u32,
    judge_calls: u32,
    recent: VecDeque<(String, Selection)>,
    updated: Instant,
    decided: Instant,
}

impl Default for Conversation {
    fn default() -> Self {
        Self {
            selected: None,
            accepted_worker: None,
            goal: String::new(),
            signal: None,
            remaining: 0,
            judge_calls: 0,
            recent: VecDeque::new(),
            updated: Instant::now(),
            decided: Instant::now(),
        }
    }
}

#[derive(Default)]
pub struct Router {
    sessions: Mutex<HashMap<String, (Instant, Arc<Mutex<Conversation>>)>>,
    failures: Mutex<HashMap<String, Instant>>,
    worker_failures: Mutex<HashMap<String, Instant>>,
}

impl Router {
    async fn conversation(
        &self,
        group: &RouteGroup,
        facts: &RequestFacts,
    ) -> Arc<Mutex<Conversation>> {
        let Some(session) = &facts.session else {
            return Arc::new(Mutex::new(Conversation::default()));
        };
        let key = format!(
            "{}\0{}\0{}\0{}\0{}",
            facts.app_type, group.id, group.revision, session, facts.branch
        );
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|_, (touched, state)| {
            touched.elapsed() < SESSION_TTL || Arc::strong_count(state) > 1
        });
        if sessions.len() >= MAX_SESSIONS && !sessions.contains_key(&key) {
            if let Some(oldest) = sessions
                .iter()
                .filter(|(_, (_, state))| Arc::strong_count(state) == 1)
                .min_by_key(|(_, (touched, _))| *touched)
                .map(|(key, _)| key.clone())
            {
                sessions.remove(&oldest);
            } else {
                return Arc::new(Mutex::new(Conversation::default()));
            }
        }
        let (touched, state) = sessions.entry(key).or_insert_with(|| {
            (
                Instant::now(),
                Arc::new(Mutex::new(Conversation::default())),
            )
        });
        *touched = Instant::now();
        state.clone()
    }

    /// The supplied judge never handles execution and can only select configured workers.
    pub async fn route<F, Fut>(
        &self,
        group: &RouteGroup,
        facts: &RequestFacts,
        judge: F,
    ) -> Result<Selection, String>
    where
        F: FnOnce(DecisionInput) -> Fut,
        Fut: Future<Output = Result<JudgeAnswer, String>>,
    {
        group.validate()?;
        if group.app_type != facts.app_type {
            return Err("Request and group belong to different clients".into());
        }
        let mut resting = self.worker_failures.lock().await;
        resting.retain(|_, failure| failure.elapsed() < FAILURE_COOLDOWN);
        let candidates: Vec<Worker> = group
            .workers
            .iter()
            .filter(|worker| {
                facts.accepts(worker) && !resting.contains_key(&Self::worker_key(group, worker))
            })
            .cloned()
            .collect();
        if candidates.is_empty() {
            let details = group
                .workers
                .iter()
                .map(|worker| {
                    let mut reasons = Vec::new();
                    if facts.needs_tools && !worker.tools {
                        reasons.push("工具调用未启用".to_string());
                    }
                    if facts.needs_images && !worker.images {
                        reasons.push("图片输入未启用".to_string());
                    }
                    if facts.estimated_tokens > worker.context_tokens {
                        reasons.push(format!(
                            "上下文不足（估算需要 {}，配置 {} token）",
                            facts.estimated_tokens, worker.context_tokens
                        ));
                    }
                    if facts.unavailable_workers.contains(&worker.id) {
                        reasons.push("供应商不可用，请检查供应商配置".to_string());
                    }
                    if let Some(failed_at) = resting.get(&Self::worker_key(group, worker)) {
                        let retry_secs = FAILURE_COOLDOWN
                            .saturating_sub(failed_at.elapsed())
                            .as_secs()
                            + 1;
                        reasons.push(format!("故障冷却，约 {retry_secs} 秒后可重试"));
                    }
                    format!("{}: {}", worker.model, reasons.join("、"))
                })
                .collect::<Vec<_>>()
                .join("；");
            return Err(format!("没有可用的工作模型。{details}。请核对工作模型能力；长会话可先压缩上下文或新建会话。"));
        }
        drop(resting);
        let legal = |id: &str| candidates.iter().find(|worker| worker.id == id).cloned();
        let fallback = legal(&group.fallback_worker)
            .or_else(|| legal(&group.default_worker))
            .or_else(|| candidates.first().cloned())
            .expect("nonempty candidates were checked above");
        let state = self.conversation(group, facts).await;
        let mut conversation = state.lock().await;
        conversation.updated = Instant::now();

        if facts.sealed_state {
            let previous = conversation.selected.as_ref().ok_or("Cannot determine the owner of incremental or encrypted state; resume on its original model")?;
            let owner = conversation.accepted_worker.as_ref().ok_or(
                "No upstream has accepted this conversation's state; automatic transfer refused",
            )?;
            let worker = legal(&owner.id)
                .ok_or("Original model cannot preserve this request; automatic transfer refused")?;
            return Ok(Selection {
                worker,
                source: DecisionSource::Continuity,
                reason: "Preserving provider-owned incremental or encrypted state".into(),
                judge_millis: 0,
                judge_input_tokens: None,
                judge_output_tokens: None,
                judge_cost_usd: None,
                ..previous.clone()
            });
        }
        if let Some((_, previous)) = conversation
            .recent
            .iter()
            .find(|(fingerprint, _)| fingerprint == &facts.fingerprint)
        {
            if let Some(worker) = legal(&previous.worker.id) {
                return Ok(Selection {
                    worker,
                    source: DecisionSource::Duplicate,
                    judge_millis: 0,
                    judge_input_tokens: None,
                    judge_output_tokens: None,
                    judge_cost_usd: None,
                    ..previous.clone()
                });
            }
        }
        let continues = matches!(
            facts.goal.trim().to_lowercase().as_str(),
            "继续"
                | "继续执行"
                | "开始执行"
                | "好"
                | "好的"
                | "go on"
                | "continue"
                | "yes"
                | "do it"
        );
        let new_goal = !facts.goal.is_empty() && !continues && facts.goal != conversation.goal;
        let stage_changed = group.mode != RouteMode::Turn
            && facts.phase_signal.as_ref().is_some_and(|signal| {
                let previous = conversation.signal.as_ref().or_else(|| {
                    conversation
                        .selected
                        .as_ref()
                        .map(|selected| &selected.phase)
                });
                previous.is_some_and(|previous| previous != signal) || facts.tool_failed
            });
        let hold_valid = !new_goal
            && !stage_changed
            && conversation.decided.elapsed() < DECISION_TTL
            && (group.mode == RouteMode::Turn || conversation.remaining > 0);
        let mut selection = if let Some(pinned) = &group.pinned_worker {
            Selection::simple(
                legal(pinned).ok_or("Pinned model cannot preserve this request")?,
                "manual",
                DecisionSource::Pinned,
                "Manually fixed model",
            )
        } else if !group.enabled {
            Selection::simple(
                legal(&group.default_worker).ok_or("Default model cannot preserve this request")?,
                "paused",
                DecisionSource::Paused,
                "Automatic routing paused",
            )
        } else if hold_valid
            && conversation
                .selected
                .as_ref()
                .is_some_and(|previous| legal(&previous.worker.id).is_some())
        {
            conversation.remaining = conversation.remaining.saturating_sub(1);
            let previous = conversation.selected.as_ref().unwrap();
            Selection {
                source: DecisionSource::Hold,
                reason: "Reusing current phase decision".into(),
                judge_millis: 0,
                judge_input_tokens: None,
                judge_output_tokens: None,
                judge_cost_usd: None,
                ..previous.clone()
            }
        } else if conversation.judge_calls >= group.max_judge_calls {
            Selection::simple(
                fallback,
                "budget",
                DecisionSource::Budget,
                "Per-conversation judge call budget exhausted",
            )
        } else {
            let judge_key = format!(
                "{}\0{}\0{}",
                group.app_type, group.judge.provider_id, group.judge.model
            );
            let cooling = self
                .failures
                .lock()
                .await
                .get(&judge_key)
                .is_some_and(|failed| failed.elapsed() < FAILURE_COOLDOWN);
            if cooling {
                Selection::simple(
                    fallback,
                    "fallback",
                    DecisionSource::Fallback,
                    "Judge is cooling down after a transport failure",
                )
            } else {
                conversation.judge_calls += 1;
                let input = DecisionInput {
                    goal: if facts.goal.is_empty() || continues {
                        conversation.goal.clone()
                    } else {
                        facts.goal.clone()
                    },
                    observation: facts.observation.clone(),
                    previous_phase: conversation
                        .selected
                        .as_ref()
                        .map(|selected| selected.phase.clone()),
                    previous_worker: conversation
                        .selected
                        .as_ref()
                        .map(|selected| selected.worker.id.clone()),
                    phase_signal: facts.phase_signal.clone(),
                    candidates: candidates.clone(),
                    max_hold_requests: group.max_hold_requests,
                };
                let start = Instant::now();
                let answer = tokio::time::timeout(
                    Duration::from_millis(group.judge_timeout_ms),
                    judge(input),
                )
                .await;
                let millis = start.elapsed().as_millis() as u64;
                match answer {
                    Ok(Ok(answer)) => {
                        self.failures.lock().await.remove(&judge_key);
                        conversation.remaining = answer
                            .hold_requests
                            .clamp(1, group.max_hold_requests)
                            .saturating_sub(1);
                        conversation.decided = Instant::now();
                        let worker = legal(&answer.worker_id);
                        let (worker, source, reason) = if let Some(worker) = worker {
                            (worker, DecisionSource::Judge, bounded(&answer.reason, 500))
                        } else {
                            (
                                fallback,
                                DecisionSource::Fallback,
                                "Judge selected an unknown or incompatible worker".into(),
                            )
                        };
                        Selection {
                            recommended_worker: answer.worker_id,
                            worker,
                            phase: bounded(&answer.phase, 80),
                            reason,
                            source,
                            judge_millis: millis,
                            judge_input_tokens: answer.input_tokens,
                            judge_output_tokens: answer.output_tokens,
                            judge_cost_usd: answer.cost_usd,
                        }
                    }
                    failure => {
                        let mut failures = self.failures.lock().await;
                        failures.retain(|_, failed| failed.elapsed() < FAILURE_COOLDOWN);
                        failures.insert(judge_key, Instant::now());
                        let reason = match failure {
                            Ok(Err(error)) => bounded(&error, 300),
                            Err(_) => "Judge timed out".into(),
                            _ => unreachable!(),
                        };
                        let mut selected = Selection::simple(
                            fallback,
                            "fallback",
                            DecisionSource::Fallback,
                            &reason,
                        );
                        selected.judge_millis = millis;
                        selected
                    }
                }
            }
        };
        if group.mode == RouteMode::Observe && selection.source == DecisionSource::Judge {
            selection.worker = legal(&group.default_worker)
                .ok_or("Observed route's default model is incompatible")?;
            selection.source = DecisionSource::Observe;
            selection.reason = format!(
                "Observation only: recommended {}; executed default model",
                selection.recommended_worker
            );
        }
        if new_goal {
            conversation.goal = facts.goal.clone();
        }
        if facts.phase_signal.is_some() {
            conversation.signal = facts.phase_signal.clone();
        }
        conversation.selected = Some(selection.clone());
        conversation
            .recent
            .push_back((facts.fingerprint.clone(), selection.clone()));
        while conversation.recent.len() > MAX_RECENT_REQUESTS {
            conversation.recent.pop_front();
        }
        Ok(selection)
    }

    /// Called only when an upstream accepted the request, including a configured fallback.
    pub async fn accepted(&self, group: &RouteGroup, facts: &RequestFacts, worker: Worker) {
        self.worker_failures
            .lock()
            .await
            .remove(&Self::worker_key(group, &worker));
        let state = self.conversation(group, facts).await;
        let mut conversation = state.lock().await;
        if conversation
            .recent
            .back()
            .is_some_and(|(fingerprint, _)| fingerprint == &facts.fingerprint)
        {
            conversation.accepted_worker = Some(worker.clone());
            if let Some(selection) = conversation.selected.as_mut() {
                selection.worker = worker.clone();
            }
        }
        if let Some((_, selection)) = conversation
            .recent
            .iter_mut()
            .find(|(fingerprint, _)| fingerprint == &facts.fingerprint)
        {
            selection.worker = worker;
        }
    }

    fn worker_key(group: &RouteGroup, worker: &Worker) -> String {
        format!(
            "{}\0{}\0{}",
            group.app_type, worker.provider_id, worker.model
        )
    }

    /// Execution fallback must respect the same cooldown as judge candidates.
    pub async fn worker_available(&self, group: &RouteGroup, worker: &Worker) -> bool {
        !self
            .worker_failures
            .lock()
            .await
            .get(&Self::worker_key(group, worker))
            .is_some_and(|failure| failure.elapsed() < FAILURE_COOLDOWN)
    }

    pub async fn failed(&self, group: &RouteGroup, worker: &Worker) {
        let mut failures = self.worker_failures.lock().await;
        failures.retain(|_, failure| failure.elapsed() < FAILURE_COOLDOWN);
        failures.insert(Self::worker_key(group, worker), Instant::now());
    }
}

impl Selection {
    fn simple(worker: Worker, phase: &str, source: DecisionSource, reason: &str) -> Self {
        Self {
            recommended_worker: worker.id.clone(),
            worker,
            phase: phase.into(),
            reason: reason.into(),
            source,
            judge_millis: 0,
            judge_input_tokens: None,
            judge_output_tokens: None,
            judge_cost_usd: None,
        }
    }
}
