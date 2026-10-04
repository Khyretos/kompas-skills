use serde_json::Value;

/// Extracts the first JSON value from text.
/// 1. Try parsing the whole trimmed text.
/// 2. If that fails, look for a ```json or ``` fence and parse its content.
/// 3. If that fails, find the first `{` or `[` and try to parse up to the matching `}` or `]`.
pub fn json_in(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    
    // 1. Try the whole text
    if let Ok(val) = serde_json::from_str(trimmed) {
        return Some(val);
    }

    // 2. Try fenced code blocks
    if let Some(val) = extract_from_fence(text) {
        return Some(val);
    }

    // 3. Try raw substring extraction
    if let Some(val) = extract_raw_substring(text) {
        return Some(val);
    }

    None
}

fn extract_from_fence(text: &str) -> Option<Value> {
    let start_marker = "```json";
    let alt_start_marker = "```";
    
    // Look for explicit json fence
    if let Some(idx) = text.find(start_marker) {
        let end_idx = text[idx + start_marker.len()..].find('`').map(|i| idx + start_marker.len() + i).unwrap_or(0);
        if end_idx > idx + start_marker.len() {
            if let Ok(val) = serde_json::from_str(&text[(idx + start_marker.len())..end_idx]) {
                return Some(val);
            }
        }
    }

    // Look for generic fence
    if let Some(idx) = text.find(alt_start_marker) {
        let end_idx = text[idx + alt_start_marker.len()..].find('`').map(|i| idx + alt_start_marker.len() + i).unwrap_or(0);
        if end_idx > idx + alt_start_marker.len() {
            if let Ok(val) = serde_json::from_str(&text[(idx + alt_start_marker.len())..end_idx]) {
                return Some(val);
            }
        }
    }

    None
}

fn extract_raw_substring(text: &str) -> Option<Value> {
    let chars: Vec<char> = text.chars().collect();
    let mut start = None;
    let mut end = None;

    for (i, c) in chars.iter().enumerate() {
        match c {
            '{' => {
                if start.is_none() {
                    start = Some(i);
                }
                if end.is_none() {
                    end = Some(i);
                }
            }
            '[' => {
                if start.is_none() {
                    start = Some(i);
                }
                if end.is_none() {
                    end = Some(i);
                }
            }
            '}' => {
                if let Some(s) = start {
                    if i == s {
                        continue;
                    }
                    // Found closing brace for opening brace
                    if end.is_none() {
                        end = Some(i);
                    }
                }
            }
            ']' => {
                if let Some(s) = start {
                    if i == s {
                        continue;
                    }
                    // Found closing bracket for opening bracket
                    if end.is_none() {
                        end = Some(i);
                    }
                }
            }
            _ => {}
        }
    }

    if let (Some(s), Some(e)) = (start, end) {
        if e > s {
            let sub = &text[s..=e];
            if let Ok(val) = serde_json::from_str(sub) {
                return Some(val);
            }
        }
    }

    None
}

/// Parses a plan from JSON or falls back to list markers.
pub fn plan(text: &str) -> Vec<String> {
    let val = json_in(text);
    
    if let Some(Value::Array(arr)) = val {
        arr.into_iter()
            .filter_map(|v| v.as_str().map(String::from))
            .take(8)
            .collect()
    } else if let Some(Value::Object(obj)) = val {
        if let Some(steps_val) = obj.get("steps").and_then(|v| v.as_array()) {
            steps_val
                .iter()
                .filter_map(|step| {
                    step.as_object().and_then(|obj| {
                        obj.get("step").or_else(|| obj.get("title")).and_then(|v| v.as_str()).map(String::from)
                    })
                })
                .take(8)
                .collect()
        } else {
            Vec::new()
        }
    } else {
        // Fallback to text lines starting with - , * , N. or N)
        fallback_plan(text)
    }
}

fn fallback_plan(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut stripped = trimmed.to_string();
        
        // Check for "- " or "* "
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            stripped = trimmed[1..].to_string();
        } 
        // Check for "N." or "N)" where N is digits
        else if let Some(stripped_part) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
             if stripped_part.starts_with('.') || stripped_part.starts_with(')') {
                 stripped = stripped_part[1..].to_string();
             }
        }

        if !stripped.is_empty() {
            result.push(stripped);
            if result.len() >= 8 {
                break;
            }
        }
    }

    result
}

#[derive(Debug, Clone)]
pub struct Review {
    pub ok: bool,
    pub findings: Vec<String>,
}

/// Reviews text for approval status.
pub fn review(text: &str) -> Review {
    let val = json_in(text);

    if let Some(Value::Object(obj)) = val {
        if let (Some(ok_val), Some(findings_val)) = (obj.get("ok"), obj.get("findings")) {
            let ok = ok_val.as_bool().unwrap_or(false);
            let findings: Vec<String> = findings_val
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            
            return Review { ok, findings };
        }
    }

    // Fallback: check for "LGTM" or "looks good"
    let lower_text = text.to_lowercase();
    let has_good = lower_text.contains("lgtm") || lower_text.contains("looks good");
    
    // ok is true only if text contains good phrase AND no findings (empty text implies no findings)
    // Since we are in fallback, we treat empty findings as implicit if not specified in JSON.
    // The rule says: "ok is true only when the text contains ... and no findings".
    // In fallback context, we assume no explicit findings unless we parsed them.
    // However, the prompt implies if we fall back, we check the text content.
    // Let's interpret "no findings" strictly: if we didn't parse findings, they are none.
    
    if has_good {
        return Review { ok: true, findings: Vec::new() };
    }

    // ok false with the whole trimmed text (first 2000 chars) as one finding
    let finding = text.trim().chars().take(2000).collect::<String>();
    Review { ok: false, findings: vec![finding] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_in_fenced() {
        let text = r#"
```json
{"key": "value"}
```
"#;
        assert!(matches!(json_in(text), Some(Value::Object(_))));
    }

    #[test]
    fn test_json_in_prose_wrapped() {
        let text = r#"Here is some text:
```
{"key": "value"}
```
More text."#;
        assert!(matches!(json_in(text), Some(Value::Object(_))));
    }

    #[test]
    fn test_plan_array() {
        let text = r#"["a", "b"]"#;
        let plan = plan(text);
        assert_eq!(plan, vec!["a", "b"]);
    }

    #[test]
    fn test_plan_object_steps() {
        let text = r#"{"steps":[{"step":"x"},{"step":"y"}]}"#;
        let plan = plan(text);
        assert_eq!(plan, vec!["x", "y"]);
    }

    #[test]
    fn test_plan_numbered_list() {
        let text = "1. one\n2. two\n3. three";
        let plan = plan(text);
        assert_eq!(plan, vec!["one", "two", "three"]);
    }

    #[test]
    fn test_review_ok_true() {
        let text = r#"{"ok":true}"#;
        let rev = review(text);
        assert!(rev.ok);
        assert!(rev.findings.is_empty());
    }

    #[test]
    fn test_review_ok_false() {
        let text = r#"{"ok":false,"findings":["f"]}"#;
        let rev = review(text);
        assert!(!rev.ok);
        assert_eq!(rev.findings, vec!["f"]);
    }

    #[test]
    fn test_review_plain_good() {
        let text = "Looks good.";
        let rev = review(text);
        assert!(rev.ok);
        assert!(rev.findings.is_empty());
    }
}
