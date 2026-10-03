use super::*;
use std::path::Path;

fn grants_with(json: &str, tag: u32) -> (std::path::PathBuf, Grants) {
    let dir = std::env::temp_dir().join(format!("kk-grants-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("grants.json");
    std::fs::write(&file, json).unwrap();
    let g = Grants::load(&file).unwrap();
    (dir, g)
}

const NOW: &str = "2026-10-03T20:00:00Z";

#[test]
fn prefix_is_component_wise() {
    let (dir, g) = grants_with(r#"[{"target":"/a/b","rights":["read"],"granted_by":"k","granted_at":"2026-10-03T18:00:00Z"}]"#, 1);
    assert!(g.allows(Path::new("/a/b/file"), &Right::Read, NOW));
    assert!(!g.allows(Path::new("/a/bc/file"), &Right::Read, NOW));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn system_grant_is_not_a_file_wildcard() {
    let (dir, g) = grants_with(r#"[{"target":"system","rights":["read"],"granted_by":"k","granted_at":"2026-10-03T18:00:00Z"}]"#, 2);
    assert!(!g.allows(Path::new("/etc/passwd"), &Right::Read, NOW));
    assert!(g.allows_system(&Right::Read, NOW));
    assert!(!g.allows_system(&Right::Shell, NOW));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expired_grant_is_skipped_not_final() {
    let (dir, g) = grants_with(
        r#"[{"target":"/a","rights":["read"],"granted_by":"k","granted_at":"2026-10-01T18:00:00Z","expires":"2026-10-02T00:00:00Z"},
            {"target":"/a/b","rights":["read"],"granted_by":"k","granted_at":"2026-10-03T18:00:00Z"}]"#,
        3,
    );
    assert!(g.allows(Path::new("/a/b/x"), &Right::Read, NOW));
    assert!(!g.allows(Path::new("/a/c"), &Right::Read, NOW));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revoke_once_and_file_round_trips() {
    let (dir, mut g) = grants_with(r#"[{"target":"/a","rights":["read"],"granted_by":"k","granted_at":"2026-10-03T18:00:00Z"}]"#, 4);
    g.add(Grant { target: "/b".into(), rights: vec![Right::Write], granted_by: "k".into(), granted_at: NOW.into(), expires: None }).unwrap();
    assert!(g.revoke("/a").unwrap());
    assert!(!g.revoke("/a").unwrap());
    let again = Grants::load(&dir.join("grants.json")).unwrap();
    assert_eq!(again.list, g.list);
    let _ = std::fs::remove_dir_all(&dir);
}
