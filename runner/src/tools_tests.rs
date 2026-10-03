use super::*;
use crate::grants::Grants;

const NOW: &str = "2026-10-03T20:00:00Z";

/// A temp dir with "inside/" granted `rights` and a sibling "outside/".
fn setup(tag: u32, rights: &str) -> (std::path::PathBuf, Grants) {
    let base = std::env::temp_dir().join(format!("kk-tools-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("inside/sub")).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    let base = std::fs::canonicalize(&base).unwrap();
    std::fs::write(base.join("inside/a.txt"), "hello").unwrap();
    std::fs::write(base.join("outside/b.txt"), "secret").unwrap();
    let file = base.join("grants.json");
    std::fs::write(
        &file,
        format!(r#"[{{"target":"{}","rights":[{rights}],"granted_by":"k","granted_at":"2026-10-03T18:00:00Z"}}]"#, base.join("inside").display()),
    )
    .unwrap();
    let g = Grants::load(&file).unwrap();
    (base, g)
}

fn p(base: &std::path::Path, rel: &str) -> String {
    base.join(rel).display().to_string()
}

#[test]
fn read_inside_outside_and_escape() {
    let (b, g) = setup(1, r#""read""#);
    let ok = run(&g, &Tool::ReadFile { path: p(&b, "inside/a.txt") }, NOW);
    assert!(ok.ok && ok.output.contains("hello"), "{}", ok.output);
    assert!(!run(&g, &Tool::ReadFile { path: p(&b, "outside/b.txt") }, NOW).ok);
    assert!(!run(&g, &Tool::ReadFile { path: p(&b, "inside/../outside/b.txt") }, NOW).ok);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn write_needs_write() {
    let (b, g) = setup(2, r#""read""#);
    assert!(!run(&g, &Tool::WriteFile { path: p(&b, "inside/new.txt"), content: "x".into() }, NOW).ok);
    let (b2, g2) = setup(3, r#""read","write""#);
    let w = run(&g2, &Tool::WriteFile { path: p(&b2, "inside/new.txt"), content: "x".into() }, NOW);
    assert!(w.ok, "{}", w.output);
    assert_eq!(std::fs::read_to_string(b2.join("inside/new.txt")).unwrap(), "x");
    let _ = std::fs::remove_dir_all(&b);
    let _ = std::fs::remove_dir_all(&b2);
}

#[test]
fn list_dir_marks_folders() {
    let (b, g) = setup(4, r#""read""#);
    let l = run(&g, &Tool::ListDir { path: p(&b, "inside") }, NOW);
    assert!(l.ok && l.output.contains("sub/") && l.output.contains("a.txt"), "{}", l.output);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn shell_needs_shell_and_reports_exit() {
    let (b, g) = setup(5, r#""read""#);
    assert!(!run(&g, &Tool::Shell { cwd: p(&b, "inside"), command: "echo hi".into() }, NOW).ok);
    let (b2, g2) = setup(6, r#""shell""#);
    let ok = run(&g2, &Tool::Shell { cwd: p(&b2, "inside"), command: "echo hi".into() }, NOW);
    assert!(ok.ok && ok.output.contains("hi") && ok.output.ends_with("exit: 0"), "{}", ok.output);
    let bad = run(&g2, &Tool::Shell { cwd: p(&b2, "inside"), command: "exit 3".into() }, NOW);
    assert!(!bad.ok && bad.output.ends_with("exit: 3"), "{}", bad.output);
    let _ = std::fs::remove_dir_all(&b);
    let _ = std::fs::remove_dir_all(&b2);
}
