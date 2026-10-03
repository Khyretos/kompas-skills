use super::*;
use crate::grants::{Grants, Right};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_grants_load() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir_all(&dir).unwrap();
    let grants_file = dir.join("grants.json");

    let grants_json = r#"[{"target":"/tmp/test","rights":["read","write"],"granted_by":"admin","granted_at":"2026-10-03T18:00:00Z","expires":"2026-10-04T18:00:00Z"}]"#;
    std::fs::write(&grants_file, grants_json).unwrap();

    let canonical_dir = dir.canonicalize().unwrap();
    let grants = Grants::load(&canonical_dir).unwrap();
    assert_eq!(grants.grants.len(), 1);
    assert_eq!(grants.grants[0].target, "/tmp/test");
    assert_eq!(grants.grants[0].rights, vec![Right::Read, Right::Write]);
    assert_eq!(grants.grants[0].granted_by, "admin");
}

#[test]
fn test_grants_allows() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir_all(&dir).unwrap();
    let grants_file = dir.join("grants.json");

    let grants_json = r#"[{"target":"/tmp/test","rights":["read","write"],"granted_by":"admin","granted_at":"2026-10-03T18:00:00Z","expires":"2026-10-04T18:00:00Z"}]"#;
    std::fs::write(&grants_file, grants_json).unwrap();

    let canonical_dir = dir.canonicalize().unwrap();
    let grants = Grants::load(&canonical_dir).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();

    assert!(grants.allows(&PathBuf::from("/tmp/test"), &Right::Read, &now));
    assert!(grants.allows(&PathBuf::from("/tmp/test/file.txt"), &Right::Read, &now));
    assert!(!grants.allows(&PathBuf::from("/tmp/other"), &Right::Read, &now));
    assert!(!grants.allows(&PathBuf::from("/tmp/test/../other"), &Right::Read, &now));
}

#[test]
fn test_grants_add_and_revoke() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir_all(&dir).unwrap();
    let grants_file = dir.join("grants.json");

    let grants_json = r#"[{"target":"/tmp/test","rights":["read"],"granted_by":"admin","granted_at":"2026-10-03T18:00:00Z","expires":"2026-10-04T18:00:00Z"}]"#;
    std::fs::write(&grants_file, grants_json).unwrap();

    let canonical_dir = dir.canonicalize().unwrap();
    let mut grants = Grants::load(&canonical_dir).unwrap();

    let new_grant = Grant {
        target: "/tmp/test".to_string(),
        rights: vec![Right::Write],
        granted_by: "user".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string(),
        expires: None,
    };

    grants.add(new_grant).unwrap();
    assert!(grants.allows(&PathBuf::from("/tmp/test"), &Right::Write, &now));

    let revoke_result = grants.revoke("tmp/test");
    assert!(revoke_result.unwrap());
    assert!(!grants.allows(&PathBuf::from("/tmp/test"), &Right::Write, &now));
}

#[test]
fn test_run_read_file() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir_all(&dir).unwrap();
    let grants_file = dir.join("grants.json");

    let grants_json = r#"[{"target":"/tmp/test","rights":["read"],"granted_by":"admin","granted_at":"2026-10-03T18:00:00Z","expires":"2026-10-04T18:00:00Z"}]"#;
    std::fs::write(&grants_file, grants_json).unwrap();

    let canonical_dir = dir.canonicalize().unwrap();
    let grants = Grants::load(&canonical_dir).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();

    let test_file = dir.join("test.txt");
    std::fs::write(&test_file, "Hello, world!").unwrap();

    let outcome = run(&grants, &Tool::ReadFile { path: test_file.to_string_lossy().to_string() }, &now);
    assert!(outcome.ok);
    assert_eq!(outcome.output, "Hello, world!");
}

#[test]
fn test_run_write_file() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir_all(&dir).unwrap();
    let grants_file = dir.join("grants.json");

    let grants_json = r#"[{"target":"/tmp/test","rights":["write"],"granted_by":"admin","granted_at":"2026-10-03T18:00:00Z","expires":"2026-10-04T18:00:00Z"}]"#;
    std::fs::write(&grants_file, grants_json).unwrap();

    let canonical_dir = dir.canonicalize().unwrap();
    let grants = Grants::load(&canonical_dir).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();

    let test_file = dir.join("test.txt");
    let outcome = run(&grants, &Tool::WriteFile { path: test_file.to_string_lossy().to_string(), content: "Hello, world!".to_string() }, &now);
    assert!(outcome.ok);
    assert!(test_file.exists());
    assert_eq!(std::fs::read_to_string(&test_file).unwrap(), "Hello, world!");
}

#[test]
fn test_run_list_dir() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir_all(&dir).unwrap();
    let grants_file = dir.join("grants.json");

    let grants_json = r#"[{"target":"/tmp/test","rights":["read"],"granted_by":"admin","granted_at":"2026-10-03T18:00:00Z","expires":"2026-10-04T18:00:00Z"}]"#;
    std::fs::write(&grants_file, grants_json).unwrap();

    let canonical_dir = dir.canonicalize().unwrap();
    let grants = Grants::load(&canonical_dir).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();

    let sub_dir = dir.join("sub");
    let _ = std::fs::create_dir(&sub_dir).unwrap();

    let outcome = run(&grants, &Tool::ListDir { path: dir.to_string_lossy().to_string() }, &now);
    assert!(outcome.ok);
    assert!(outcome.output.contains("sub/"));
}

#[test]
fn test_run_shell() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = std::fs::create_dir_all(&dir).unwrap();
    let grants_file = dir.join("grants.json");

    let grants_json = r#"[{"target":"/tmp/test","rights":["shell"],"granted_by":"admin","granted_at":"2026-10-03T18:00:00Z","expires":"2026-10-04T18:00:00Z"}]"#;
    std::fs::write(&grants_file, grants_json).unwrap();

    let canonical_dir = dir.canonicalize().unwrap();
    let grants = Grants::load(&canonical_dir).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();

    let outcome = run(&grants, &Tool::Shell { cwd: dir.to_string_lossy().to_string(), command: "echo hi".to_string() }, &now);
    assert!(outcome.ok);
    assert!(outcome.output.contains("hi"));
    assert!(outcome.output.ends_with("exit: 0"));

    let outcome = run(&grants, &Tool::Shell { cwd: dir.to_string_lossy().to_string(), command: "exit 3".to_string() }, &now);
    assert!(!outcome.ok);
    assert!(outcome.output.ends_with("exit: 3"));
}
