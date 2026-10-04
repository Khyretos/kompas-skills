//! "Used in" (milestone 5): which assets the games' scenes place. kk-engine scenes
//! (`*.scene.json`) name their packs ("packs": ["PolygonTown_Source_Files"]) and each object's
//! asset by file stem (objects[].asset, optional objects[].pack and objects[].texture). The
//! library's pack names differ ("POLYGON_Town_SourceFiles_v5"), so both sides are compared
//! by a normalised key. Rebuilt from the repos (read-only) after every scan.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde_json::Value;
use sqlx::SqlitePool;

use super::games::{repo_name, repos};

/// Folders never searched for scenes: build output, third-party code, and the git-ignored
/// asset packs themselves.
const SKIP_DIRS: &[&str] = &[".git", "assets", "external", "third_party", "node_modules", "target", "website", "out", ".cache"];
const MAX_DEPTH: usize = 6;
/// Model formats, best first, when a scene's stem matches several files.
const MODEL_EXTS: &[&str] = &["fbx", "glb", "gltf", "obj", "blend", "dae", "prefab"];

/// Scene files in a repo, relative to it, sorted.
pub fn scene_files(repo: &Path) -> Vec<String> {
    let mut out = vec![];
    let mut stack: Vec<(PathBuf, usize)> = vec![(repo.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            if t.is_dir() {
                if depth < MAX_DEPTH && !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with("build") {
                    stack.push((e.path(), depth + 1));
                }
            } else if t.is_file() && name.ends_with(".scene.json")
                && let Ok(rel) = e.path().strip_prefix(repo)
            {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// The game a scene belongs to: `games/<dir>/...` is that game, anything else the repo's
/// shared scenes.
pub fn game_key_of(repo: &str, scene: &str) -> String {
    match scene.strip_prefix("games/").and_then(|r| r.split_once('/')) {
        Some((dir, _)) => format!("{repo}/{dir}"),
        None => format!("{repo}/scenes"),
    }
}

/// "POLYGON_Town_SourceFiles_v5", "PolygonTown_Source_Files" and "POLYGON Town (1)" all
/// become "polygontown".
pub fn pack_key(name: &str) -> String {
    let mut s: String = name.to_lowercase();
    // A re-download's " (1)".
    if let Some(i) = s.rfind(" (")
        && s.ends_with(')')
        && s[i + 2..s.len() - 1].chars().all(|c| c.is_ascii_digit())
    {
        s.truncate(i);
    }
    let mut s: String = s.chars().filter(|c| c.is_alphanumeric()).collect();
    // A version at the end: v2, v1.03 (dots are gone by now).
    if let Some(i) = s.rfind('v')
        && i > 0
        && s.len() > i + 1
        && s[i + 1..].chars().all(|c| c.is_ascii_digit())
    {
        s.truncate(i);
    }
    for word in ["sourcefiles", "sourcefile", "source", "files", "unitypackage", "unity"] {
        if let Some(rest) = s.strip_suffix(word) {
            s = rest.to_string();
        }
    }
    s
}

/// What one scene file names.
#[derive(Debug, Default, PartialEq)]
pub struct Scene {
    pub packs: Vec<String>,
    /// (asset stem, the object's own pack if any, texture file if any), with placements.
    pub objects: Vec<(String, Option<String>, Option<String>, i64)>,
}

pub fn parse_scene(text: &str) -> Option<Scene> {
    let v: Value = serde_json::from_str(text).ok()?;
    let packs = v["packs"].as_array().map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect()).unwrap_or_default();
    let mut counts: HashMap<(String, Option<String>, Option<String>), i64> = HashMap::new();
    for o in v["objects"].as_array().into_iter().flatten() {
        let Some(asset) = o["asset"].as_str().map(str::trim).filter(|a| !a.is_empty()) else { continue };
        let pack = o["pack"].as_str().map(String::from).filter(|p| !p.is_empty());
        let texture = o["texture"].as_str().map(String::from).filter(|t| !t.is_empty());
        *counts.entry((asset.to_string(), pack, texture)).or_default() += 1;
    }
    let mut objects: Vec<_> = counts.into_iter().map(|((a, p, t), n)| (a, p, t, n)).collect();
    objects.sort();
    Some(Scene { packs, objects })
}

fn stem(name: &str) -> &str {
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

fn ext_rank(name: &str) -> usize {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    MODEL_EXTS.iter().position(|e| *e == ext).unwrap_or(MODEL_EXTS.len())
}

/// The library, indexed for matching: packs by key, assets by (pack, lowercase stem) and
/// by lowercase stem alone.
struct Library {
    packs: HashMap<String, Vec<i64>>,
    by_pack: HashMap<(i64, String), Vec<(i64, String)>>,
    by_stem: HashMap<String, Vec<(i64, i64, String)>>,
    by_file: HashMap<(i64, String), i64>,
}

async fn library(db: &SqlitePool) -> Result<Library> {
    let packs: Vec<(i64, String, Option<i64>)> =
        sqlx::query_as("SELECT id, name, duplicate_of FROM asset_pack WHERE missing_since IS NULL ORDER BY duplicate_of IS NOT NULL, id")
            .fetch_all(db)
            .await?;
    let mut by_key: HashMap<String, Vec<i64>> = HashMap::new();
    for (id, name, _) in packs {
        by_key.entry(pack_key(&name)).or_default().push(id);
    }
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, pack_id, name FROM asset WHERE missing_since IS NULL AND category <> 'junk' AND is_meta = 0 AND dup_of IS NULL",
    )
    .fetch_all(db)
    .await?;
    let mut lib = Library { packs: by_key, by_pack: HashMap::new(), by_stem: HashMap::new(), by_file: HashMap::new() };
    for (id, pack, name) in rows {
        let s = stem(&name).to_lowercase();
        lib.by_pack.entry((pack, s.clone())).or_default().push((id, name.clone()));
        lib.by_stem.entry(s).or_default().push((id, pack, name.clone()));
        lib.by_file.insert((pack, name.to_lowercase()), id);
    }
    Ok(lib)
}

impl Library {
    fn packs_named(&self, name: &str) -> Vec<i64> {
        self.packs.get(&pack_key(name)).cloned().unwrap_or_default()
    }

    /// The asset an object places: its own pack first, then the scene's packs in order,
    /// then anywhere in the library if exactly one pack has that name. Models before other
    /// files of the same stem (fbx before obj).
    fn find(&self, asset: &str, own_pack: Option<&str>, scene_packs: &[i64]) -> Option<(i64, i64)> {
        let s = asset.to_lowercase();
        let own: Vec<i64> = own_pack.map(|p| self.packs_named(p)).unwrap_or_default();
        for pack in own.iter().chain(scene_packs) {
            if let Some(list) = self.by_pack.get(&(*pack, s.clone())) {
                return list.iter().min_by_key(|(_, n)| ext_rank(n)).map(|(id, _)| (*id, *pack));
            }
        }
        let all = self.by_stem.get(&s)?;
        let packs: HashSet<i64> = all.iter().map(|(_, p, _)| *p).collect();
        (packs.len() == 1).then(|| all.iter().min_by_key(|(_, _, n)| ext_rank(n)).map(|(id, p, _)| (*id, *p))).flatten()
    }
}

/// Rebuilds "used in" for every mounted repo. A repo that isn't mounted keeps its rows.
pub async fn run(db: &SqlitePool) -> Result<(usize, usize)> {
    let lib = library(db).await?;
    let games: HashMap<String, i64> = sqlx::query_as::<_, (String, i64)>("SELECT key, id FROM asset_game").fetch_all(db).await?.into_iter().collect();
    let (mut scenes_read, mut uses) = (0, 0);
    for repo in repos() {
        if !repo.is_dir() {
            continue;
        }
        let name = repo_name(&repo);
        let mut tx = db.begin().await?;
        let prefix = format!("{name}/%");
        for table in ["asset_use", "asset_pack_use", "asset_scene_miss"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE game_id IN (SELECT id FROM asset_game WHERE key LIKE ?)"))
                .bind(&prefix)
                .execute(&mut *tx)
                .await?;
        }
        for scene in scene_files(&repo) {
            let Some(&game) = games.get(&game_key_of(&name, &scene)) else { continue };
            let Some(parsed) = std::fs::read_to_string(repo.join(&scene)).ok().as_deref().and_then(parse_scene) else {
                tracing::warn!(scene, "scene file unreadable");
                continue;
            };
            scenes_read += 1;
            let mut scene_packs = vec![];
            for p in &parsed.packs {
                let found = lib.packs_named(p);
                match found.first() {
                    Some(&id) => {
                        scene_packs.push(id);
                        sqlx::query("INSERT OR IGNORE INTO asset_pack_use (game_id, pack_id, scene) VALUES (?, ?, ?)")
                            .bind(game)
                            .bind(id)
                            .bind(&scene)
                            .execute(&mut *tx)
                            .await?;
                    }
                    None => miss(&mut tx, game, &scene, "pack", p).await?,
                }
            }
            for (asset, own_pack, texture, count) in &parsed.objects {
                let Some((id, pack)) = lib.find(asset, own_pack.as_deref(), &scene_packs) else {
                    miss(&mut tx, game, &scene, "asset", asset).await?;
                    continue;
                };
                uses += 1;
                add_use(&mut tx, game, id, &scene, *count).await?;
                if let Some(t) = texture
                    && let Some(&tid) = lib.by_file.get(&(pack, t.to_lowercase()))
                {
                    add_use(&mut tx, game, tid, &scene, *count).await?;
                }
            }
        }
        tx.commit().await?;
    }
    Ok((scenes_read, uses))
}

async fn add_use(tx: &mut sqlx::SqliteConnection, game: i64, asset: i64, scene: &str, count: i64) -> Result<()> {
    sqlx::query(
        "INSERT INTO asset_use (game_id, asset_id, scene, count) VALUES (?, ?, ?, ?)
         ON CONFLICT(game_id, asset_id, scene) DO UPDATE SET count = count + excluded.count",
    )
    .bind(game)
    .bind(asset)
    .bind(scene)
    .bind(count)
    .execute(tx)
    .await?;
    Ok(())
}

async fn miss(tx: &mut sqlx::SqliteConnection, game: i64, scene: &str, kind: &str, name: &str) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO asset_scene_miss (game_id, scene, kind, name) VALUES (?, ?, ?, ?)")
        .bind(game)
        .bind(scene)
        .bind(kind)
        .bind(name)
        .execute(tx)
        .await?;
    Ok(())
}

/// Where an asset is used: [{ gameId, game, scenes: [{ scene, count }] }].
pub async fn used_in(db: &SqlitePool, asset: i64) -> Result<Value> {
    let rows: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT g.id, g.name, u.scene, u.count FROM asset_use u JOIN asset_game g ON g.id = u.game_id
         WHERE u.asset_id = ? ORDER BY g.name COLLATE NOCASE, u.scene",
    )
    .bind(asset)
    .fetch_all(db)
    .await?;
    let mut out: Vec<Value> = vec![];
    for (gid, game, scene, count) in rows {
        let entry = serde_json::json!({ "scene": scene, "count": count });
        match out.last_mut() {
            Some(last) if last["gameId"] == gid => last["scenes"].as_array_mut().unwrap().push(entry),
            _ => out.push(serde_json::json!({ "gameId": gid, "game": game, "scenes": [entry] })),
        }
    }
    Ok(Value::from(out))
}

/// A game's scene summary for the Games tab.
pub async fn summary(db: &SqlitePool, game: i64) -> Result<Value> {
    let (assets, packs, scenes): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(DISTINCT asset_id) FROM asset_use WHERE game_id = ?1),
                (SELECT COUNT(DISTINCT pack_id) FROM asset_pack_use WHERE game_id = ?1),
                (SELECT COUNT(DISTINCT scene) FROM (SELECT scene FROM asset_use WHERE game_id = ?1
                     UNION SELECT scene FROM asset_pack_use WHERE game_id = ?1 UNION SELECT scene FROM asset_scene_miss WHERE game_id = ?1))",
    )
    .bind(game)
    .fetch_one(db)
    .await?;
    let missing: Vec<(String, String, String)> =
        sqlx::query_as("SELECT kind, name, scene FROM asset_scene_miss WHERE game_id = ? ORDER BY kind DESC, name LIMIT 50")
            .bind(game)
            .fetch_all(db)
            .await?;
    Ok(serde_json::json!({
        "assets": assets, "packs": packs, "scenes": scenes,
        "missing": missing.into_iter().map(|(k, n, s)| serde_json::json!({ "kind": k, "name": n, "scene": s })).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_names_match_across_spellings() {
        for n in ["POLYGON_Town_SourceFiles_v5", "PolygonTown_Source_Files", "POLYGON_Town", "POLYGON Town (1)"] {
            assert_eq!(pack_key(n), "polygontown", "{n}");
        }
        assert_eq!(pack_key("POLYGON_Nature_Source_Files_v2"), "polygonnature");
        assert_ne!(pack_key("POLYGON_NatureBiomes_MeadowForest_SourceFiles_v3"), "polygonnature");
        assert_eq!(pack_key("Fantasy RPG Music Pack Vol.3"), "fantasyrpgmusicpackvol3", "a volume number is not a version");
    }

    #[test]
    fn scenes_belong_to_their_game() {
        assert_eq!(game_key_of("kk-engine", "games/showcase/course_art.scene.json"), "kk-engine/showcase");
        assert_eq!(game_key_of("kk-engine", "scenes/forest_trail.scene.json"), "kk-engine/scenes");
    }

    #[test]
    fn scene_objects_are_counted() {
        let s = parse_scene(
            r#"{"format": "kke.scene", "packs": ["POLYGON_Nature"], "objects": [
                {"asset": "SM_Tree_01", "position": [0, 0, 0]}, {"asset": "SM_Tree_01", "position": [5, 0, 0]},
                {"asset": "SM_Rock_01", "pack": "POLYGON_Town", "texture": "PolygonTown_Texture_02.png"},
                {"position": [1, 1, 1]}]}"#,
        )
        .unwrap();
        assert_eq!(s.packs, ["POLYGON_Nature"]);
        assert_eq!(
            s.objects,
            vec![
                ("SM_Rock_01".into(), Some("POLYGON_Town".into()), Some("PolygonTown_Texture_02.png".into()), 1),
                ("SM_Tree_01".into(), None, None, 2)
            ]
        );
        assert_eq!(parse_scene("not json"), None);
    }

    #[tokio::test]
    async fn used_in_from_a_repo() {
        // A repo with a game scene and a shared scene; the library has two packs.
        let repo = std::env::temp_dir().join(format!("kk-engine-scenes-{}", std::process::id()));
        let name = repo_name(&repo);
        std::fs::create_dir_all(repo.join("games/forest")).unwrap();
        std::fs::create_dir_all(repo.join("scenes")).unwrap();
        std::fs::create_dir_all(repo.join("assets/synty")).unwrap();
        std::fs::write(repo.join("games/forest/game.json"), r#"{"title": "Forest"}"#).unwrap();
        std::fs::write(
            repo.join("games/forest/trail.scene.json"),
            r#"{"packs": ["POLYGON_Nature"], "objects": [{"asset": "SM_Tree_01"}, {"asset": "SM_Tree_01"}, {"asset": "SM_Gone_01"}]}"#,
        )
        .unwrap();
        std::fs::write(
            repo.join("scenes/town.scene.json"),
            r#"{"packs": ["PolygonTown_Source_Files", "POLYGON_Missing"], "objects": [{"asset": "SM_Bld_Shop_01", "texture": "Town_Tex_02.png"}]}"#,
        )
        .unwrap();
        std::fs::write(repo.join("assets/synty/ignored.scene.json"), "{}").unwrap();
        assert_eq!(scene_files(&repo), ["games/forest/trail.scene.json", "scenes/town.scene.json"]);

        super::super::ai::register_sqlite_vec();
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        sqlx::query("INSERT INTO asset_pack (id, key, name, kind) VALUES (1, 'n.zip', 'POLYGON_Nature_Source_Files_v2', 'zip'), (2, 't.zip', 'POLYGON_Town_SourceFiles_v5', 'zip')")
            .execute(&db)
            .await
            .unwrap();
        for (id, pack, name) in [
            (1, 1, "SM_Tree_01.obj"),
            (2, 1, "SM_Tree_01.fbx"),
            (3, 2, "SM_Bld_Shop_01.fbx"),
            (4, 2, "Town_Tex_02.png"),
            (5, 2, "SM_Tree_01.fbx"),
        ] {
            sqlx::query("INSERT INTO asset (id, pack_id, container, path, name, ext, size, category, rule) VALUES (?, ?, ?, ?, ?, 'x', 1, '3d-model', 't')")
                .bind(id)
                .bind(pack)
                .bind(format!("p{pack}.zip"))
                .bind(name)
                .bind(name)
                .execute(&db)
                .await
                .unwrap();
        }
        for (key, gname) in [(format!("{name}/forest"), "Forest"), (format!("{name}/scenes"), "Shared scenes")] {
            sqlx::query("INSERT INTO asset_game (key, name, source, created_at) VALUES (?, ?, ?, 'now')").bind(key).bind(gname).bind(&name).execute(&db).await.unwrap();
        }
        // SAFETY: tests in this module run on their own; GAME_REPOS is only read here.
        unsafe { std::env::set_var("GAME_REPOS", repo.to_str().unwrap()) };
        let (scenes, uses) = run(&db).await.unwrap();
        unsafe { std::env::remove_var("GAME_REPOS") };
        assert_eq!((scenes, uses), (2, 2));
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT g.name, u.asset_id, u.count FROM asset_use u JOIN asset_game g ON g.id = u.game_id ORDER BY g.name, u.asset_id",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![("Forest".into(), 2, 2), ("Shared scenes".into(), 3, 1), ("Shared scenes".into(), 4, 1)],
            "the fbx from the scene's own pack (not the obj, not the other pack's tree); the texture variant too"
        );
        let missing: Vec<(String, String)> = sqlx::query_as("SELECT kind, name FROM asset_scene_miss ORDER BY name").fetch_all(&db).await.unwrap();
        assert_eq!(missing, vec![("pack".into(), "POLYGON_Missing".into()), ("asset".into(), "SM_Gone_01".into())]);
        let used = used_in(&db, 2).await.unwrap();
        assert_eq!(used[0]["game"], "Forest");
        assert_eq!(used[0]["scenes"][0]["count"], 2);
        std::fs::remove_dir_all(&repo).unwrap();
    }
}
