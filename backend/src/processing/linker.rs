use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ingestion::{commits::GitHubCommit, issues::Issue, pull_requests::PullRequest};

#[derive(Debug, Serialize, Deserialize)]
pub struct EventNode {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub event_type: EventType,
    pub title: String,
    pub description: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub source_ref: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum EventType {
    Commit,
    PullRequest,
    Issue,
}

impl EventType {
    pub fn as_str(self) -> &'static str {
        match self {
            EventType::Commit => "commit",
            EventType::PullRequest => "pull_request",
            EventType::Issue => "issue",
        }
    }
}

pub struct EventGraph {
    pub nodes: Vec<EventNode>,
}

impl EventGraph {
    pub fn new() -> Self {
        EventGraph { nodes: Vec::new() }
    }

    pub fn add_node(&mut self, node: EventNode) {
        self.nodes.push(node);
    }

    pub fn sort_chronological(&mut self) {
        self.nodes.sort_by_key(|n| n.timestamp);
    }

    /// Compact, prompt-friendly projection of the timeline — no internal ids.
    pub fn to_payload(&self) -> Value {
        Value::Array(
            self.nodes
                .iter()
                .map(|n| {
                    json!({
                        "type": n.event_type.as_str(),
                        "source": &n.source_ref,
                        "title": &n.title,
                        "author": &n.author,
                        "timestamp": n.timestamp.to_rfc3339(),
                    })
                })
                .collect(),
        )
    }
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))
}

/// Build a chronological event timeline from ingested GitHub data. This is the
/// ordering signal the analysis model needs: the same facts, arranged by time,
/// so it can reason about what happened before what — decisions, debates, and
/// reversals unfold in sequence rather than as an unordered dump.
pub fn build_timeline(
    repo_id: Uuid,
    commits: &[GitHubCommit],
    prs:     &[PullRequest],
    issues:  &[Issue],
) -> EventGraph {
    let mut graph = EventGraph::new();

    for c in commits {
        if let Some(ts) = parse_ts(&c.commit.author.date) {
            graph.add_node(EventNode {
                id: Uuid::new_v4(),
                repo_id,
                event_type: EventType::Commit,
                title: c.commit.message.lines().next().unwrap_or("").to_string(),
                description: c.commit.message.clone(),
                author: c.author.as_ref().and_then(|a| a.login.as_deref()).unwrap_or("unknown").to_string(),
                timestamp: ts,
                source_ref: format!("commit:{}", &c.sha[..c.sha.len().min(12)]),
            });
        }
    }

    for pr in prs {
        if let Some(ts) = parse_ts(&pr.created_at) {
            graph.add_node(EventNode {
                id: Uuid::new_v4(),
                repo_id,
                event_type: EventType::PullRequest,
                title: format!("#{} {}", pr.number, pr.title),
                description: pr.body.as_deref().unwrap_or("").to_string(),
                author: pr.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown").to_string(),
                timestamp: ts,
                source_ref: format!("pr:{}", pr.number),
            });
        }
    }

    for i in issues {
        if let Some(ts) = parse_ts(&i.created_at) {
            graph.add_node(EventNode {
                id: Uuid::new_v4(),
                repo_id,
                event_type: EventType::Issue,
                title: format!("#{} {}", i.number, i.title),
                description: i.body.as_deref().unwrap_or("").to_string(),
                author: i.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown").to_string(),
                timestamp: ts,
                source_ref: format!("issue:{}", i.number),
            });
        }
    }

    graph.sort_chronological();
    graph
}
