pub fn smart_truncate(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines { return content.to_string(); }
    let mut result = Vec::new();
    for line in lines.iter().take(max_lines) {
        result.push(*line);
    }
    result.push("// ... (truncated)");
    result.join("\n")
}
