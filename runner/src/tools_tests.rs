use super::*;
use crate::grants::{Grants, Right};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_grants_load() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir(&dir);
    let grants_file = dir.join("grants.json");
    let grants_content = r#"[{"target":"/tmp/testdir","rights":["read","write"],"granted_by":"admin","granted_at":"1620000000","expires":"1620000000"}]"#;
    std::fs::write(&grants_file, grants_content).unwrap();
    let grants = Grants::load(&dir).unwrap();
    assert!(grants.allows(&Path::new("/tmp/testdir"), &Right::Read, "1620000000"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_grants_allows() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir(&dir);
    let grants_file = dir.join("grants.json");
    let grants_content = r#"[{"target":"/tmp/testdir","rights":["read"],"granted_by":"admin","granted_at":"1620000000","expires":"1620000000"}]"#;
    std::fs::write(&grants_file, grants_content).unwrap();
    let grants = Grants::load(&dir).unwrap();
    assert!(grants.allows(&Path::new("/tmp/testdir"), &Right::Read, "1620000000"));
    assert!(!grants.allows(&Path::new("/tmp/testdir/sibling"), &Right::Read, "1620000000"));
    assert!(!grants.allows(&Path::new("/tmp/testdir/../outside"), &Right::Read, "1620000000"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_grants_add_and_revoke() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir(&dir);
    let grants_file = dir.join("grants.json");
    let mut grants = Grants::load(&dir).unwrap();
    let grant = crate::grants::Grant {
        target: "/tmp/testdir".to_string(),
        rights: vec!["read".to_string(), "write".to_string()],
        granted_by: "admin".to_string(),
        granted_at: "1620000000".to_string(),
        expires: Some("1620000000".to_string()),
    };
    grants.add(grant).unwrap();
    assert!(grants.allows(&Path::new("/tmp/testdir"), &Right::Read, "1620000000"));
    assert!(grants.revoke("/tmp/testdir").unwrap());
    assert!(!grants.allows(&Path::new("/tmp/testdir"), &Right::Read, "1620000000"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_run_read_file() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir(&dir);
    let grants_file = dir.join("grants.json");
    let grants_content = r#"[{"target":"/tmp/testdir","rights":["read"],"granted_by":"admin","granted_at":"1620000000","expires":"1620000000"}]"#;
    std::fs::write(&grants_file, grants_content).unwrap();
    let grants = Grants::load(&dir).unwrap();
    let test_file = dir.join("testfile.txt");
    std::fs::write(&test_file, "test content").unwrap();
    let outcome = run(&grants, &Tool::ReadFile { path: test_file.to_string_lossy().to_string() }, "1620000000");
    assert!(outcome.ok);
    assert_eq!(outcome.output, "test content");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_run_write_file() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir(&dir);
    let grants_file = dir.join("grants.json");
    let grants_content = r#"[{"target":"/tmp/testdir","rights":["write"],"granted_by":"admin","granted_at":"1620000000","expires":"1620000000"}]"#;
    std::fs::write(&grants_file, grants_content).unwrap();
    let grants = Grants::load(&dir).unwrap();
    let outcome = run(&grants, &Tool::WriteFile { path: dir.join("newfile.txt").to_string_lossy().to_string(), content: "new content".to_string() }, "1620000000");
    assert!(outcome.ok);
    assert!(std::fs::metadata(&dir.join("newfile.txt")).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_run_list_dir() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir(&dir);
    let grants_file = dir.join("grants.json");
    let grants_content = r#"[{"target":"/tmp/testdir","rights":["read"],"granted_by":"admin","granted_at":"1620000000","expires":"1620000000"}]"#;
    std::fs::write(&grants_file, grants_content).unwrap();
    let grants = Grants::load(&dir).unwrap();
    let sub_dir = dir.join("subdir");
    let _ = std::fs::create_dir(&sub_dir);
    let outcome = run(&grants, &Tool::ListDir { path: dir.to_string_lossy().to_string() }, "1620000000");
    assert!(outcome.ok);
    assert!(outcome.output.contains("subdir/"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_run_shell() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir(&dir);
    let grants_file = dir.join("grants.json");
    let grants_content = r#"[{"target":"/tmp/testdir","rights":["shell"],"granted_by":"admin","granted_at":"1620000000","expires":"1620000000"}]"#;
    std::fs::write(&grants_file, grants_content).unwrap();
    let grants = Grants::load(&dir).unwrap();
    let outcome = run(&grants, &Tool::Shell { cwd: dir.to_string_lossy().to_string(), command: "echo hi".to_string() }, "1620000000");
    assert!(outcome.ok);
    assert!(outcome.output.contains("hi"));
    assert!(outcome.output.ends_with("exit: 0"));
    let outcome = run(&grants, &Tool::Shell { cwd: dir.to_string_lossy().to_string(), command: "exit 3".to_string() }, "1620000000");
    assert!(!outcome.ok);
    assert!(outcome.output.ends_with("exit: 3"));
    let outcome = run(&grants, &Tool::Shell { cwd: dir.to_string_lossy().to_string(), command: "exit 0".to_string() }, "1620000000");
    assert!(outcome.ok);
    assert!(outcome.output.ends_with("exit: 0"));
    let outcome = run(&grants, &Tool::Shell { cwd: dir.to_string_lossy().to_string(), command: "invalid command".to_string() }, "1620000000");
    assert!(!outcome.ok);
    assert!(outcome.output.contains("not granted"));
    let _ = std::fs::remove_dir_all(&dir);
}
