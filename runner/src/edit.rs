use std::{fs, path::Path, path::PathBuf};
use crate::grants::{Grants, Right};
use crate::tools::Outcome;

pub fn edit_file(grants: &Grants, path: &str, old: &str, new: &str, now: &str) -> Outcome {
    let Ok(file) = Path::new(path).canonicalize() else {
        return no("no such file");
    };

    if !file.is_file() {
        return no("not a file");
    }

    if !grants.allows(&file, &Right::Write, now) {
        return no(format!("not granted: write on {}", file.display()));
    }

    const MAX_SIZE: u64 = 1024 * 1024; // 1 MiB
    let mut before = String::new();
    if fs::metadata(&file).map(|m| m.len()).unwrap_or(0) > MAX_SIZE {
        return no("file too large");
    }
    let Ok(before) = fs::read_to_string(&file) else {
        return no("failed to read file");
    };

    if old.is_empty() {
        return no("old text must not be empty");
    }

    let count = before.matches(old).count();
    if count != 1 {
        return no(format!("old text found {} times", count));
    }

    let after = before.replacen(old, new, 1);
    
    let tmp_path = file.with_file_name(format!("{}.kompanion-tmp", file.file_name().unwrap_or_default().to_string_lossy()));
    
    match fs::write(&tmp_path, &after) {
        Ok(_) => {
            if let Ok(meta) = fs::metadata(&file) {
                if let Err(e) = fs::set_permissions(&tmp_path, meta.permissions()) {
                    let _ = fs::remove_file(&tmp_path);
                    return no(format!("failed to copy permissions: {}", e));
                }
            }
            if let Err(e) = fs::rename(&tmp_path, &file) {
                let _ = fs::remove_file(&tmp_path);
                return no(format!("failed to rename: {}", e));
            }
            Outcome { ok: true, output: diff_lines(&before, &after) }
        },
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            no(format!("failed to write temp: {}", e))
        }
    }
}

fn no(s: impl Into<String>) -> Outcome {
    Outcome { ok: false, output: s.into() }
}

pub fn diff_lines(a: &str, b: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let mut p = 0; 
    while p < a.len() && p < b.len() && a[p] == b[p] { 
        p += 1; 
    }
    let mut s = 0; 
    while s < a.len() - p && s < b.len() - p && a[a.len()-1-s] == b[b.len()-1-s] { 
        s += 1; 
    }
    if p == a.len() && p == b.len() { 
        return String::new(); 
    }
    let mut result = format!("@@ line {} @@\n", p + 1);
    for line in &a[p..a.len()-s] {
        result.push_str(&format!("-{}\n", line));
    }
    for line in &b[p..b.len()-s] {
        result.push_str(&format!("+{}\n", line));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::process;

    fn setup_test_dir() -> PathBuf {
        let dir = env::temp_dir().join(format!("kk-edit-{}", process::id()));
        fs::create_dir_all(&dir).expect("Failed to create test dir");
        dir
    }

    fn cleanup(dir: &Path) {
        if dir.exists() {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn test_diff_lines_equal() {
        let a = "line1\nline2\nline3";
        let b = "line1\nline2\nline3";
        assert_eq!(diff_lines(a, b), "");
    }

    #[test]
    fn test_diff_lines_middle_change() {
        let a = "line1\nb\nline3";
        let b = "line1\nB\nline3";
        let expected = "@@ line 2 @@\n-b\n+B\n";
        assert_eq!(diff_lines(a, b), expected);
    }

    #[test]
    fn test_diff_lines_added_last() {
        let a = "line1\nline2";
        let b = "line1\nline2\nnew";
        let expected = "+new\n";
        assert_eq!(diff_lines(a, b), expected);
    }

    #[test]
    fn test_edit_file_success() {
        let dir = setup_test_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "hello world").expect("Failed to write test file");
        
        let grants_path = dir.join("grants.json");
        let grants_json = serde_json::json!([{"target": format!("{}/", dir.display()), "rights": ["write"], "granted_by": "test", "granted_at": "2026-01-01T00:00:00Z"}]).to_string();
        fs::write(&grants_path, &grants_json).expect("Failed to write grants file");
        
        let grants = Grants::load(&grants_path).expect("Failed to load grants");
        let now = "2026-06-01T00:00:00Z";
        
        let outcome = edit_file(&grants, &file.to_string_lossy(), "hello", "goodbye", now);
        assert!(outcome.ok);
        assert!(fs::read_to_string(&file).unwrap().contains("goodbye"));
        cleanup(&dir);
    }

    #[test]
    fn test_edit_file_old_text_twice() {
        let dir = setup_test_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "hello hello").expect("Failed to write test file");
        
        let grants_path = dir.join("grants.json");
        let grants_json = serde_json::json!([{"target": format!("{}/", dir.display()), "rights": ["write"], "granted_by": "test", "granted_at": "2026-01-01T00:00:00Z"}]).to_string();
        fs::write(&grants_path, &grants_json).expect("Failed to write grants file");
        
        let grants = Grants::load(&grants_path).expect("Failed to load grants");
        let now = "2026-06-01T00:00:00Z";
        
        let outcome = edit_file(&grants, &file.to_string_lossy(), "hello", "goodbye", now);
        assert!(!outcome.ok);
        assert!(outcome.output.contains("found 2 times"));
        assert_eq!(fs::read_to_string(&file).unwrap(), "hello hello");
        cleanup(&dir);
    }

    #[test]
    fn test_edit_file_no_grant() {
        let dir = setup_test_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "hello").expect("Failed to write test file");
        
        let grants_path = dir.join("grants.json");
        let grants_json = serde_json::json!([{"target": "/other/dir", "rights": ["write"], "granted_by": "test", "granted_at": "2026-01-01T00:00:00Z"}]).to_string();
        fs::write(&grants_path, &grants_json).expect("Failed to write grants file");
        
        let grants = Grants::load(&grants_path).expect("Failed to load grants");
        let now = "2026-06-01T00:00:00Z";
        
        let outcome = edit_file(&grants, &file.to_string_lossy(), "hello", "goodbye", now);
        assert!(!outcome.ok);
        assert!(outcome.output.contains("not granted"));
        cleanup(&dir);
    }
}
