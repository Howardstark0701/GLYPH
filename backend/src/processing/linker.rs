use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Serialize, Deserialize)]
pub enum EventType {
    Commit,
    PullRequest,
    Issue,
    Review,
    Merge,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventEdge {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub relation: String,
}

pub struct EventGraph {
    pub nodes: Vec<EventNode>,
    pub edges: Vec<EventEdge>,
}

impl EventGraph {
    pub fn new() -> Self {
        EventGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: EventNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: EventEdge) {
        self.edges.push(edge);
    }

    pub fn sort_chronological(&mut self) {
        self.nodes.sort_by_key(|n| n.timestamp);
    }

    pub fn find_node(&self, source_ref: &str) -> Option<&EventNode> {
        self.nodes.iter().find(|n| n.source_ref == source_ref)
    }
}
