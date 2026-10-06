use std::path::Path;

/// Returns the list of files named by a shell command string.
/// 
/// Splits on whitespace, filters out flags, operators, and non-file tokens,
/// stripping quotes and leading "./". Returns unique files in order.
pub fn check_files(cmd: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for token in cmd.split_whitespace() {
        // Strip surrounding single or double quotes (only if length >= 2)
        let token = if token.len() >= 2 {
            if token.starts_with('"') && token.ends_with('"') {
                &token[1..token.len()-1]
            } else if token.starts_with('\'') && token.ends_with('\'') {
                &token[1..token.len()-1]
            } else {
                token
            }
        } else {
            token
        };

        // Skip flags, redirections, and variable assignments
        if token.contains('=') || token.contains('>') || token.contains('<') {
            continue;
        }

        // Skip flags
        if token.starts_with('-') {
            continue;
        }

        // Skip "." and ".."
        if token == "." || token == ".." {
            continue;
        }

        // Skip shell operators and commands that don't look like files.
        // A file token must contain '.' or '/' after the first character.
        // This excludes operators like |, &, ;, >, <, (, ), {, }, [, ], ` 
        // and commands like cd, ls, python3 which lack these characters.
        if !token.contains('.') && !token.contains('/') {
            continue;
        }

        // Strip leading "./"
        let token = token.strip_prefix("./").unwrap_or(token);

        // Deduplicate preserving order
        if seen.insert(token.to_string()) {
            result.push(token.to_string());
        }
    }

    result
}

/// Checks if a given path is protected based on naming conventions or extra rules.
/// 
/// Returns true if the file matches test_* patterns, *_test patterns,
/// ends with .spec.ts or .test.ts, or resides in a 'tests' directory,
/// or matches an entry in the extra list.
pub fn is_protected(path: &str, extra: &[String]) -> bool {
    // Normalize path: strip leading "./" if present for comparison logic
    let normalized_path = path.strip_prefix("./").unwrap_or(path);
    
    // Check extra rules first
    for entry in extra {
        let entry_normalized = entry.strip_prefix("./").unwrap_or(entry);
        if entry_normalized == normalized_path {
            return true;
        }
        if normalized_path.ends_with(&format!("/{}", entry_normalized)) {
            return true;
        }
    }

    // Check directory components for "tests"
    let parts: Vec<&str> = normalized_path.split('/').collect();
    if parts.iter().any(|p| *p == "tests") {
        return true;
    }

    // Check filename patterns
    let stem = Path::new(normalized_path).file_name().and_then(|n| n.to_str()).unwrap_or("");
    
    if stem.starts_with("test_") && stem.ends_with(".py") {
        return true;
    }
    if stem.ends_with("_test.py") || stem.ends_with("_test.go") {
        return true;
    }
    if stem.ends_with(".spec.ts") || stem.ends_with(".test.ts") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_files_example() {
        let cmd = "cd app && python3 -m pytest test_names.py ./check.sh";
        let files = check_files(cmd);
        assert_eq!(files, vec!["test_names.py", "check.sh"]);
    }

    #[test]
    fn test_is_protected_absolute_py() {
        assert!(is_protected("/home/u/proj/test_names.py", &[]));
    }

    #[test]
    fn test_is_protected_relative_py() {
        assert!(is_protected("proj/test_names.py", &[]));
    }

    #[test]
    fn test_is_protected_not_regular_py() {
        assert!(!is_protected("/home/u/proj/names.py", &[]));
    }

    #[test]
    fn test_is_protected_json_in_tests_dir() {
        assert!(is_protected("/p/tests/data.json", &[]));
    }

    #[test]
    fn test_is_protected_shell_in_extra() {
        assert!(is_protected("/p/src/check.sh", &["check.sh".to_string()]));
    }

    #[test]
    fn test_is_protected_not_in_extra() {
        assert!(!is_protected("/p/src/recheck.sh", &["check.sh".to_string()]));
    }
}
