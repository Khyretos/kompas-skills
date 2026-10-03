use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_grants_add_and_revoke() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = fs::create_dir_all(&dir).map_err(|e| e.to_string());
    let dir = dir.canonicalize().unwrap();

    let mut grants = Grants::load(&dir).unwrap();
    let grant = Grant {
        target: "/a/b".to_string(),
        rights: vec![Right::Read, Right::Write],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        expires: None,
    };
    grants.add(grant).unwrap();

    assert!(grants.allows(&dir.join("a/b"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(grants.allows(&dir.join("a/b"), &Right::Write, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.allows(&dir.join("a/bc/file"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));

    let revoked = grants.revoke("/a/b").unwrap();
    assert!(revoked);
    assert!(!grants.allows(&dir.join("a/b"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.revoke("/a/b").unwrap());
}

#[test]
fn test_expired_grants() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = fs::create_dir_all(&dir).map_err(|e| e.to_string());
    let dir = dir.canonicalize().unwrap();

    let mut grants = Grants::load(&dir).unwrap();
    let grant = Grant {
        target: "/a/b".to_string(),
        rights: vec![Right::Read],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        expires: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 100),
    };
    grants.add(grant).unwrap();

    let grant2 = Grant {
        target: "/a/b".to_string(),
        rights: vec![Right::Read],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 100,
        expires: None,
    };
    grants.add(grant2).unwrap();

    assert!(!grants.allows(&dir.join("a/b"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(grants.allows(&dir.join("a/b"), &Right::Read, &(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 200).to_string()));
}

#[test]
fn test_grants_prefix_check() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = fs::create_dir_all(&dir).map_err(|e| e.to_string());
    let dir = dir.canonicalize().unwrap();

    let mut grants = Grants::load(&dir).unwrap();
    let grant = Grant {
        target: "/a/b".to_string(),
        rights: vec![Right::Read],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        expires: None,
    };
    grants.add(grant).unwrap();

    assert!(grants.allows(&dir.join("a/b"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.allows(&dir.join("a/bc/file"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.allows(&dir.join("a/bc"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
}

#[test]
fn test_system_grants() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let _ = fs::create_dir_all(&dir).map_err(|e| e.to_string());
    let dir = dir.canonicalize().unwrap();

    let mut grants = Grants::load(&dir).unwrap();
    let grant = Grant {
        target: "system".to_string(),
        rights: vec![Right::Read],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        expires: None,
    };
    grants.add(grant).unwrap();

    assert!(grants.allows_system(&Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.allows(&dir.join("any/path"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
}
