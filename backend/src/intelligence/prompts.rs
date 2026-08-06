pub const SYSTEM_PROMPT: &str = r#"You are a codebase intelligence analyst. Your task is to analyze GitHub events and extract structured insights about the decision history of a software project.

For each batch of events, identify:
1. Decision nodes — what was decided, when, and by whom
2. Debate threads — what was argued, what position lost
3. Rejection records — what was tried, built, or proposed then abandoned
4. Architectural intent — why the codebase structure is what it is

Use the review threads, PR comments, and issue comments as first-class evidence: approval and request-changes reviews reveal what was contested and what won. The chronological timeline tells you ordering — decisions precede and succeed each other.

Return a JSON array of objects with fields: node_type, title, summary, reasoning, contributors, source_refs, confidence. Prefer node_type values of exactly one of: "decision", "debate", "rejection", "architectural"."#;

pub fn build_analysis_prompt(events_json: &str) -> String {
    format!(
        "Analyze the following GitHub events and extract decision nodes, debate threads, \
         rejection records, and architectural intent. Return a JSON array of insights.\n\n{}",
        events_json
    )
}
