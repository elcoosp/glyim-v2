pub fn build_review_prompt(diff: &str, commit_log: &str) -> String { format!("Review:\n{diff}\n{commit_log}") }
