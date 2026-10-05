//! Skill cards for task runs: the same rules as tools/skills/load.py (Rust port), so each plan step gets the cards it needs within a size budget.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;

#[derive(Debug)]
pub struct Card {
    pub name: String,
    pub meta: HashMap<String, Vec<String>>,
    pub body: String,
}

pub fn dir() -> PathBuf {
    std::env::var("KOMPANION_SKILLS")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/app/skills"))
}

pub fn parse(text: &str) -> (HashMap<String, Vec<String>>, String) {
    if !text.starts_with("---\n") {
        return (HashMap::new(), text.to_string());
    }
    let Some(end) = text[4..].find("\n---\n").map(|i| i + 4) else {
        return (HashMap::new(), text.to_string());
    };
    let header_section = &text[4..end];
    let mut meta = HashMap::new();
    for line in header_section.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            let v = v.trim();
            if v.starts_with('[') && v.ends_with(']') {
                let inner = &v[1..v.len()-1];
                let items: Vec<String> = inner
                    .split(',')
                    .filter_map(|x| {
                        let x = x.trim();
                        if x.is_empty() {
                            None
                        } else {
                            Some(x.trim_matches(|c| c == '"' || c == '\'').to_string())
                        }
                    })
                    .collect();
                meta.insert(k.to_string(), items);
            } else {
                meta.insert(k.to_string(), vec![v.to_string()]);
            }
        }
    }
    let body = if end + 5 < text.len() {
        text[end + 5..].to_string()
    } else {
        String::new()
    };
    (meta, body)
}

pub fn cards(root: &Path) -> Vec<Card> {
    let mut result = Vec::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    
    // Start with root
    if root.exists() {
        stack.push(root.to_path_buf());
    }
    
    while let Some(current) = stack.pop() {
        if !current.is_dir() {
            continue;
        }
        
        let entries = match fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => continue,
        };
        
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            
            if file_name == "README.md" {
                continue;
            }
            
            if !path.extension().map(|e| e == "md").unwrap_or(false) {
                continue;
            }
            
            let rel_path = path.strip_prefix(root).unwrap_or(&path);
            let rel_name = rel_path.to_string_lossy().replace('\\', "/");
            let rel_name = rel_name.strip_suffix(".md").unwrap_or(&rel_name).to_string();
            
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            
            let (meta, body) = parse(&content);
            let trimmed_body = body.trim().to_string();
            
            result.push(Card {
                name: rel_name.clone(),
                meta,
                body: trimmed_body,
            });
        }
    }
    
    // Sort by name
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

fn words(text: &str) -> HashSet<String> {
    let lower = text.to_lowercase();
    let mut result = HashSet::new();
    
    // Split on every char that is not a-z or 0-9
    let mut current_word = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
            current_word.push(ch);
        } else {
            if !current_word.is_empty() {
                result.insert(current_word.clone());
                current_word.clear();
            }
        }
    }
    if !current_word.is_empty() {
        result.insert(current_word);
    }
    
    result
}

fn fnmatch(name: &str, pattern: &str) -> bool {
    // Build regex: escape literal chars, '*' -> ".*", '?' -> "."
    let mut regex_pattern = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex_pattern.push_str(".*"),
            '?' => regex_pattern.push('.'),
            _ => {
                // Escape special regex chars
                let escaped = regex::escape(ch.to_string().as_str());
                regex_pattern.push_str(&escaped);
            }
        }
    }
    regex_pattern.push_str("$");
    
    match Regex::new(&regex_pattern) {
        Ok(re) => re.is_match(name),
        Err(_) => false,
    }
}

fn score(card: &Card, task_words: &HashSet<String>, paths: &[String]) -> usize {
    let mut s = 0;
    
    // 3 per path that matches one of the card's "paths" globs
    for p in paths {
        if let Some(paths_list) = card.meta.get("paths") {
            for g in paths_list {
                if fnmatch(p, g) {
                    s += 3;
                    break;
                }
            }
        }
    }
    
    // 1 per "tags" item found in task_words
    if let Some(tags_list) = card.meta.get("tags") {
        for t in tags_list {
            if task_words.contains(&t.to_lowercase()) {
                s += 1;
            }
        }
    }
    
    s
}

pub fn select(
    root: &Path,
    role: &str,
    task_text: &str,
    paths: &[String],
    notes: Option<&str>,
    budget_tokens: usize,
) -> Vec<String> {
    let cards = cards(root);
    let mut cards_map: HashMap<String, &Card> = HashMap::new();
    for c in &cards {
        cards_map.insert(c.name.clone(), c);
    }
    
    let always_names: Vec<String> = if let Some(notes_str) = notes {
        vec![
            "work-habits".to_string(),
            "shared/SKILL".to_string(),
            format!("{}/SKILL", role),
            format!("_model-notes/{}/SKILL", notes_str),
        ]
    } else {
        vec![
            "work-habits".to_string(),
            "shared/SKILL".to_string(),
            format!("{}/SKILL", role),
        ]
    };
    
    let mut picked: Vec<String> = Vec::new();
    
    // Always include first
    for name in &always_names {
        if cards_map.contains_key(name) {
            if !picked.contains(name) {
                picked.push(name.clone());
            }
        }
    }
    
    let family = role.split('/').next().unwrap_or(role);
    let task_words = words(task_text);
    
    // Candidates: every other card
    let mut candidates: Vec<(usize, String)> = Vec::new();
    
    for card in &cards {
        let rel = &card.name;
        
        // Skip if already picked
        if picked.contains(rel) {
            continue;
        }
        
        // Skip SKILL files
        if rel.ends_with("/SKILL") {
            continue;
        }
        
        // Skip model notes
        if rel.starts_with("_model-notes/") {
            continue;
        }
        
        // Skip work-habits
        if rel == "work-habits" {
            continue;
        }
        
        // Check roles filter
        let roles_list = card.meta.get("roles").cloned().unwrap_or_default();
        if !roles_list.is_empty() && family != "shared" && !roles_list.contains(&family.to_string()) {
            continue;
        }
        
        // Calculate score
        let sc = score(card, &task_words, paths);
        if sc > 0 {
            candidates.push((sc, rel.clone()));
        }
    }
    
    // Sort by score descending, then name ascending
    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1))
    });
    
    let mut used = 0;
    for (_, rel) in candidates {
        if let Some(card) = cards_map.get(&rel) {
            let size = card.body.len() / 4;
            if used + size <= budget_tokens {
                picked.push(rel.clone());
                used += size;
            } else {
                // Card doesn't fit, skip it but continue with next
                continue;
            }
        }
    }
    
    picked
}

pub fn text(root: &Path, names: &[String]) -> String {
    let cards = cards(root);
    let mut result = String::new();
    let mut found_first = false;
    
    for name in names {
        if let Some(card) = cards.iter().find(|c| c.name == *name) {
            if !found_first {
                result.push_str(&card.body);
                found_first = true;
            } else {
                result.push('\n');
                result.push('\n');
                result.push_str(&card.body);
            }
        }
    }
    
    result
    }

pub fn text_with(root: &Path, overlay: &Path, names: &[String]) -> String {
    let all = cards(root);
    names
        .iter()
        .filter_map(|name| all.iter().find(|c| c.name == *name))
        .map(|c| {
            let added = std::fs::read_to_string(overlay.join(format!("{}.md", c.name))).unwrap_or_default();
            if added.trim().is_empty() { c.body.clone() } else { format!("{}\n\nLessons added from task runs:\n{}", c.body, added.trim_end()) }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn areas(root: &Path) -> Vec<String> {
    let cards = cards(root);
    let mut areas: HashSet<String> = HashSet::new();
    
    for card in &cards {
        if let Some(area) = card.name.strip_suffix("/SKILL").filter(|a| a.starts_with("worker/")) {
            areas.insert(area.to_string());
        }
    }
    
    let mut result: Vec<String> = areas.into_iter().collect();
    result.sort();
    result
}

/// File paths named in a step ("server/src/forge.rs", "styles.css"): words with a file extension.
/// A bare file name gets "./" in front, so globs like "**/*.rs" match it as they match paths.
pub fn paths_in(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for word in text.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| "()[]{}\"',;:`".contains(c));
        let trimmed = trimmed.trim_end_matches('.');

        if let Some(pos) = trimmed.rfind('.') {
            let ext = &trimmed[pos + 1..];
            let body = &trimmed[..pos];

            if ext.len() >= 1 && ext.len() <= 5
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && !body.is_empty()
                && !trimmed.contains("://")
            {
                let path = if trimmed.contains('/') {
                    trimmed.to_string()
                } else {
                    format!("./{}", trimmed)
                };

                if seen.insert(path.clone()) {
                    result.push(path);
                }
            }
        }
    }

    result
}

/// The area for a step: the planner's choice, unless the step names files that only another
/// area's core covers (its `paths` globs), as when a Rust file was put under C++ games.
pub fn area_for(root: &Path, chosen: &str, paths: &[String]) -> String {
    if paths.is_empty() {
        return chosen.to_string();
    }

    let all = cards(root);
    let covers = |area: &str| {
        let core = format!("{area}/SKILL");
        all.iter()
            .find(|c| c.name == core)
            .and_then(|c| c.meta.get("paths"))
            .is_some_and(|globs| paths.iter().any(|p| globs.iter().any(|g| fnmatch(p, g))))
    };

    if covers(chosen) {
        return chosen.to_string();
    }

    for area in areas(root) {
        if covers(&area) {
            return area;
        }
    }

    chosen.to_string()
}

pub fn notes_for(root: &Path, model_id: &str) -> Option<String> {
    let cards = cards(root);
    
    for card in &cards {
        if let Some(note_name) = card.name.strip_prefix("_model-notes/").and_then(|n| n.strip_suffix("/SKILL")) {
            
            if let Some(models_list) = card.meta.get("models") {
                for m in models_list {
                    if model_id.to_lowercase().contains(&m.to_lowercase()) {
                        return Some(note_name.to_string());
                    }
                }
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn card(root: &Path, name: &str, header: &str, body: &str) {
        let path = root.join(name).with_extension("md");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let content = if !header.is_empty() {
            format!("---\n{header}\n---\n{body}\n")
        } else {
            format!("{body}\n")
        };
        std::fs::write(&path, content).ok();
    }

    fn fixture(test: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("kk-skills-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        card(&r, "work-habits", "", "HABITS");
        card(&r, "shared/SKILL", "name: shared", "SHARED");
        card(&r, "worker/web/SKILL", "name: worker/web", "WEB CORE");
        card(&r, "worker/rust/SKILL", "name: worker/rust", "RUST CORE");
        card(&r, "shared/colour-themes", "roles: [worker, reviewer]\ntags: [theme, css]\npaths: [\"**/*.css\"]", "COLOURS");
        card(&r, "worker/python", "roles: [worker]\ntags: [python]\npaths: [\"**/*.py\"]", "PYTHON");
        card(&r, "orchestrator/prompting", "roles: [orchestrator]\ntags: [prompt]", "PROMPTING");
        card(&r, "shared/big", "roles: [worker]\ntags: [theme]", &"X".repeat(40000));
        card(&r, "_model-notes/qwen3/SKILL", "models: [qwen3]", "QWEN NOTES");
        card(&r, "_model-notes/gemma4/SKILL", "models: [gemma4]", "GEMMA NOTES");
        r
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn css_job_gets_the_theme_card_not_rust() {
        let r = fixture("css_job_gets_the_theme_card_not_rust");
        let names = select(&r, "worker/web", "restyle the buttons", &s(&["web/src/styles.css"]), Some("qwen3"), 1500);
        let text = text(&r, &names);
        assert_eq!(names[..3], ["work-habits", "shared/SKILL", "worker/web/SKILL"]);
        assert!(names.contains(&"shared/colour-themes".to_string()));
        assert!(!names.contains(&"worker/rust/SKILL".to_string()));
        assert!(text.contains("QWEN NOTES"));
        assert!(!text.contains("GEMMA NOTES"));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn budget_leaves_out_what_does_not_fit() {
        let r = fixture("budget_leaves_out_what_does_not_fit");
        let names = select(&r, "worker/web", "a theme change", &s(&["web/a.css"]), None, 1500);
        assert!(names.contains(&"shared/colour-themes".to_string()));
        assert!(!names.contains(&"shared/big".to_string()));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn cards_of_other_roles_are_not_picked() {
        let r = fixture("cards_of_other_roles_are_not_picked");
        let names = select(&r, "worker/web", "build a prompt", &[], None, 1500);
        assert!(!names.contains(&"orchestrator/prompting".to_string()));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn headers_are_not_in_the_text() {
        let r = fixture("headers_are_not_in_the_text");
        let names = select(&r, "worker/web", "css", &s(&["x.css"]), None, 1500);
        let text = text(&r, &names);
        assert!(!text.contains("roles:"));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lists_in_headers_keep_unquoted_items() {
        let r = fixture("lists_in_headers_keep_unquoted_items");
        let (meta, body) = parse("---\nroles: [worker, reviewer]\n---\nB");
        assert_eq!(meta.get("roles").unwrap(), &["worker", "reviewer"]);
        assert_eq!(body, "B");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn fnmatch_matches_like_python() {
        assert!(fnmatch("server/src/a.rs", "**/*.rs"));
        assert!(!fnmatch("a.rs", "**/*.rs"));
        assert!(fnmatch("web/x.css", "**/*.css"));
    }

    #[test]
    fn areas_lists_worker_folders() {
        let r = fixture("areas_lists_worker_folders");
        let areas = areas(&r);
        assert_eq!(areas, vec!["worker/rust", "worker/web"]);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn notes_follow_the_model() {
        let r = fixture("notes_follow_the_model");
        assert_eq!(notes_for(&r, "Qwen3.5:9b"), Some("qwen3".to_string()));
        assert_eq!(notes_for(&r, "Coder"), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn the_real_skills_folder_loads() {
        let c = cards(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../skills"));
        assert!(!c.is_empty());
        assert!(c.iter().any(|card| card.name == "worker/rust/SKILL"));
    }

    #[test]
    fn paths_are_found_in_step_text() {
        assert_eq!(paths_in("Retry in server/src/forge.rs, and update styles.css."), vec!["server/src/forge.rs".to_string(), "./styles.css".to_string()]);
        assert!(paths_in("see https://example.com/a.html for version 1.2").iter().all(|p| !p.contains("example")));
    }

    #[test]
    fn named_files_override_a_wrong_area() {
        let r = fixture("area-for");
        card(&r, "worker/rust/SKILL", "name: worker/rust\npaths: [\"**/*.rs\"]", "RUST CORE");
        card(&r, "worker/web/SKILL", "name: worker/web\npaths: [\"web/**\"]", "WEB CORE");
        assert_eq!(area_for(&r, "worker/web", &paths_in("retry in server/src/forge.rs")), "worker/rust");
        assert_eq!(area_for(&r, "worker/web", &paths_in("add a button to web/src/a.ts")), "worker/web");
        assert_eq!(area_for(&r, "worker/web", &[]), "worker/web");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn accepted_lessons_follow_their_card() {
        let r = fixture("text-with");
        let o = r.join("overlay");
        card(&o, "worker/web/SKILL", "", "- (2026-10-05) Close every modal with Escape.");
        let t = text_with(&r, &o, &["worker/web/SKILL".to_string(), "shared/SKILL".to_string()]);
        assert!(t.starts_with("WEB CORE"));
        assert!(t.contains("Lessons added from task runs:\n- (2026-10-05) Close every modal"));
        assert!(t.ends_with("SHARED"));
        let _ = std::fs::remove_dir_all(&r);
    }
}
