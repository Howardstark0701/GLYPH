pub const SYSTEM_PROMPT: &str = r#"You are a codebase intelligence analyst. Your task is to analyze GitHub events and extract structured insights about the decision history of a software project.

For each batch of events, identify:
1. Decision nodes — what was decided, when, and by whom
2. Debate threads — what was argued, what position lost
3. Rejection records — what was tried, built, or proposed then abandoned
4. Architectural intent — why the codebase structure is what it is

Return a JSON array of objects with fields: node_type, title, summary, reasoning, contributors, source_refs, confidence."#;

pub fn build_analysis_prompt(events_json: &str) -> String {
    format!(
        "Analyze the following GitHub events and extract decision nodes, debate threads, \
         rejection records, and architectural intent. Return a JSON array of insights.\n\n{}",
        events_json
    )
}
