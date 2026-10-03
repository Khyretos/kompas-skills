use super::*;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_grants_load_missing_file() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let grants = Grants::load(&dir).unwrap();
    assert!(grants.list.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_grants_add_and_revoke() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let dir_path = dir.canonicalize().unwrap();
    let mut grants = Grants::load(&dir).unwrap();

    let grant = Grant {
        target: dir_path.to_string_lossy().to_string(),
        rights: vec![Right::Read, Right::Write],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string(),
        expires: None,
    };

    grants.add(grant.clone()).unwrap();
    assert!(grants.allows(&dir_path, &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(grants.allows(&dir_path, &Right::Write, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.allows(&dir_path, &Right::Shell, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));

    let revoked = grants.revoke(&grant.target).unwrap();
    assert!(revoked);
    assert!(!grants.allows(&dir_path, &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.revoke(&grant.target).unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_grants_prefix_check() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let dir_path = dir.canonicalize().unwrap();
    let mut grants = Grants::load(&dir).unwrap();

    let grant = Grant {
        target: "/a/b".to_string(),
        rights: vec![Right::Read],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string(),
        expires: None,
    };

    grants.add(grant).unwrap();
    assert!(grants.allows(&Path::new("/a/b/file.txt"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.allows(&Path::new("/a/bc/file.txt"), &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_system_grants() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let dir_path = dir.canonicalize().unwrap();
    let mut grants = Grants::load(&dir).unwrap();

    let grant = Grant {
        target: "system".to_string(),
        rights: vec![Right::Read],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string(),
        expires: None,
    };

    grants.add(grant).unwrap();
    assert!(grants.allows_system(&Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));
    assert!(!grants.allows(&dir_path, &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_expired_grants() {
    let dir = std::env::temp_dir().join(format!("kk-{}-{}", std::process::id(), line!()));
    let dir_path = dir.canonicalize().unwrap();
    let mut grants = Grants::load(&dir).unwrap();

    let grant = Grant {
        target: dir_path.to_string_lossy().to_string(),
        rights: vec![Right::Read],
        granted_by: "test".to_string(),
        granted_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string(),
        expires: Some((SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 60).to_string()),
    };

    grants.add(grant).unwrap();
    assert!(!grants.allows(&dir_path, &Right::Read, &SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}
