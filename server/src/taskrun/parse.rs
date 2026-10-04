use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct PlanStep {
    pub what: String,
    pub done_when: String,
}

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

/// One plan item: a string, or an object with "step"/"title" and an optional
/// "done_when"/"doneWhen"/"done". None when the step text is empty.
fn plan_item(v: &Value) -> Option<PlanStep> {
    let field = |keys: &[&str]| keys.iter().find_map(|k| v.get(*k).and_then(Value::as_str)).unwrap_or("").trim().to_string();
    let (what, done_when) = match v {
        Value::String(s) => (s.trim().to_string(), String::new()),
        Value::Object(_) => (field(&["step", "title"]), field(&["done_when", "doneWhen", "done"])),
        _ => return None,
    };
    (!what.is_empty()).then_some(PlanStep { what, done_when })
}

/// Returns true if the step only looks at something; the worker does that anyway.
fn look_only(what: &str) -> bool {
    let lower = what.to_lowercase();
    let first_word = lower.split_whitespace().next().unwrap_or("").to_string();

    // Verbs that are purely observational
    const OBSERVE: &[&str] = &["open", "read", "locate", "find", "look", "view", "inspect", "examine", "review"];
    if OBSERVE.contains(&first_word.as_str()) {
        return true;
    }

    // "run/verify/confirm/check" + "test"/"check"
    if ["run", "verify", "confirm", "check"].contains(&first_word.as_str())
        && (lower.contains("test") || lower.contains("check"))
    {
        return true;
    }

    false
}

/// Parses a plan from JSON (an array of steps, or {"steps": [...]}) or falls back to list
/// markers. Look-only steps are dropped unless nothing else is left.
pub fn plan(text: &str) -> Vec<PlanStep> {
    let steps: Vec<PlanStep> = match json_in(text) {
        Some(Value::Array(arr)) => arr.iter().filter_map(plan_item).take(8).collect(),
        Some(Value::Object(obj)) => match obj.get("steps") {
            Some(Value::Array(arr)) => arr.iter().filter_map(plan_item).take(8).collect(),
            _ => Vec::new(),
        },
        _ => fallback_plan(text).into_iter().map(|what| PlanStep { what, done_when: String::new() }).take(8).collect(),
    };
    let real: Vec<PlanStep> = steps.iter().filter(|s| !look_only(&s.what)).cloned().collect();
    if real.is_empty() { steps } else { real }
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
            // Trim again to remove any trailing spaces left after stripping the marker
            stripped = stripped.trim().to_string();
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
        if let Some(ok_val) = obj.get("ok") {
            let ok = ok_val.as_bool().unwrap_or(false);
            
            // Handle findings: if missing, treat as empty list
            let findings_val = obj.get("findings");
            let findings: Vec<String> = match findings_val {
                Some(v) => v
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| item.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            
            return Review { ok, findings };
        }
    }

    // Fallback: check for "LGTM" or "looks good"
    let lower_text = text.to_lowercase();
    let has_good = lower_text.contains("lgtm") || lower_text.contains("looks good");
    
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
        assert_eq!(plan, vec![PlanStep { what: "a".to_string(), done_when: String::new() }, PlanStep { what: "b".to_string(), done_when: String::new() }]);
    }

    #[test]
    fn test_plan_object_steps() {
        let text = r#"{"steps":[{"step":"x"},{"step":"y"}]}"#;
        let plan = plan(text);
        assert_eq!(plan, vec![PlanStep { what: "x".to_string(), done_when: String::new() }, PlanStep { what: "y".to_string(), done_when: String::new() }]);
    }

    #[test]
    fn test_plan_numbered_list() {
        let text = "1. one\n2. two\n3. three";
        let plan = plan(text);
        assert_eq!(plan, vec![PlanStep { what: "one".to_string(), done_when: String::new() }, PlanStep { what: "two".to_string(), done_when: String::new() }, PlanStep { what: "three".to_string(), done_when: String::new() }]);
    }

    #[test]
    fn test_plan_with_done_when() {
        let text = r#"[{"step":"add char_count","done_when":"char_count(\"a b\") == 2"}]"#;
        let plan = plan(text);
        assert_eq!(plan, vec![PlanStep { what: "add char_count".to_string(), done_when: "char_count(\"a b\") == 2".to_string() }]);
    }

    #[test]
    fn test_plan_alternate_done_when_keys() {
        let text = r#"{"steps":[{"title":"x","doneWhen":"y"}]}"#;
        let plan = plan(text);
        assert_eq!(plan, vec![PlanStep { what: "x".to_string(), done_when: "y".to_string() }]);
    }

    #[test]
    fn look_only_steps_are_dropped() {
        let whats = |t: &str| plan(t).into_iter().map(|s| s.what).collect::<Vec<_>>();
        // The demo run's planner wrote an "open and locate" step and a "run the tests" step.
        assert_eq!(
            whats(r#"["Open names.py and locate full_name", "Add initials() below full_name", "Run python3 -m unittest to check"]"#),
            ["Add initials() below full_name"]
        );
        assert_eq!(whats("1. Find the route\n2. Add the route"), ["Add the route"]);
        // Nothing else left: keep them.
        assert_eq!(whats(r#"["Read the config"]"#), ["Read the config"]);
        // Running something that is not a test is a real step.
        assert_eq!(whats(r#"["Run npm install", "Add the route"]"#), ["Run npm install", "Add the route"]);
    }

    #[test]
    fn test_plan_skip_empty_steps() {
        let text = r#"["a", {"step":"  "}, "b"]"#;
        let plan = plan(text);
        assert_eq!(plan, vec![PlanStep { what: "a".to_string(), done_when: String::new() }, PlanStep { what: "b".to_string(), done_when: String::new() }]);
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
