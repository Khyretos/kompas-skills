//! What an asset is, from its path alone. Port of `classify()` in
//! Services/ai/ai-skills/asset-library/survey_assets.py (same rules, same test cases).
//!
//! Files inside a zip are classified as `<zip name>/<entry>`: the pack name is often
//! the only hint ("Fantasy RPG Music Pack.zip" holds `Tracks/mp3/Action 1.mp3`).

use std::sync::LazyLock;

use regex::Regex;

/// Extension -> broad kind. The word rules below refine kinds into categories.
const KIND: &[(&str, &str)] = &[
    ("audio", "wav ogg mp3 flac aif aiff m4a aac opus wma"),
    ("model", "bvh fbx obj gltf glb blend dae 3ds max ma mb ply usd usda usdc usdz abc x3d"),
    // 3D-print files: never game assets, but they share drives with them.
    ("print", "stl 3mf lys chitubox ctb gcode"),
    ("image", "png jpg jpeg tga tif tiff psd bmp gif webp exr hdr dds ktx ktx2 svg kra xcf"),
    ("shader", "glsl vert frag comp hlsl shader shadergraph shadersubgraph cginc wgsl spv"),
    ("font", "ttf otf woff woff2 fnt"),
    ("video", "mp4 mov webm avi mkv"),
    ("material", "mat mtl sbsar sbs"),
    (
        "engine",
        "unitypackage prefab unity asset meta controller anim overridecontroller mask \
         physicmaterial uasset umap tscn tres scn gd",
    ),
    ("archive", "zip rar 7z tar gz tgz xz bz2"),
    ("doc", "txt md pdf html htm rtf doc docx json xml csv yml yaml"),
];

const DEFAULT_CAT: &[(&str, &str)] = &[
    ("audio", "sound-effect"),
    ("model", "3d-model"),
    ("print", "print-model"),
    ("image", "image"),
    ("shader", "shader"),
    ("font", "font"),
    ("video", "video"),
    ("material", "material"),
    ("engine", "engine-file"),
    ("archive", "archive"),
    ("doc", "doc"),
];

/// Every category, in the order the UI lists them.
pub const CATEGORIES: &[&str] = &[
    "sound-effect", "music", "ambience", "voice", "3d-model", "animation", "texture", "material",
    "sprite", "vfx", "image", "shader", "font", "video", "print-model", "engine-file", "archive",
    "doc", "other", "junk",
];

/// A word rule: the path must match `path`; with `name`, the file name must match too.
struct Rule {
    category: &'static str,
    path: Regex,
    name: Option<Regex>,
}

fn rule(category: &'static str, path: &str, name: Option<&str>) -> Rule {
    Rule { category, path: Regex::new(path).unwrap(), name: name.map(|n| Regex::new(n).unwrap()) }
}

/// Ordered rules per kind, first match wins. Regexes run on the lower-case path.
static WORDS: LazyLock<Vec<(&'static str, Vec<Rule>)>> = LazyLock::new(|| {
    vec![
        (
            "audio",
            vec![
                // Music first: music packs name loops "for dialogues" and tracks "(No Vocals)".
                rule(
                    "music",
                    r"(\bmusic|song|bgm|soundtrack|\bost\b|\btracks?\b|theme|melod|piano|chiptune|tunes|synthwave|lofi)",
                    None,
                ),
                // Whole words only: "vo" must not match "Vol. 2", "Void" or "Voltage". (Python uses a
                // lookahead for the end; the regex crate has none, and consuming it matches the same.)
                rule(
                    "voice",
                    r"(^|[/_\- ])(voices?|vo|vox|dialog|dialogue|speech|narrat\w*)($|[/_\- .\d])",
                    None,
                ),
                rule("ambience", r"(ambien|ambience|atmos|\bamb[_ \-]|room ?tone|background loop)", None),
            ],
        ),
        (
            "model",
            vec![
                rule("animation", r"(^|/)(anim|anims|animation|animations|mocap)/.*\.(fbx|glb|gltf|dae|blend)$", None),
                // Only rigged formats hold animation; STL/OBJ/PLY are static. Words on the file
                // name, not the folder: "Presupport@hehestl/" is a print-shop folder.
                rule(
                    "animation",
                    r"\.(fbx|glb|gltf|blend|dae|abc)$",
                    Some(r"(^a_|^anim_|@|anim|mocap|idle|walk|run|jump|attack)"),
                ),
            ],
        ),
        (
            "image",
            vec![
                rule("vfx", r"(vfx|particle|flipbook|explosion|smoke|\bfire\b|spark|(^|/)fx/)", None),
                rule("sprite", r"(sprite|tileset|tilemap|/ui/|icon|2d|pixel|portrait)", None),
                rule(
                    "texture",
                    r"(textur|/tex/|albedo|basecolor|base_color|normal|rough|metal|_ao\b|occlusion|emissi|height|mask|diffuse|specular|_n\.|_d\.)",
                    None,
                ),
            ],
        ),
    ]
});

/// OS and tool leftovers: counted as "junk" so they don't clutter "other".
static JUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(^|/)(__macosx/|\._|\.ds_store$|desktop\.ini$|thumbs\.db$|\.mayaswatches/)|\.(bak|swatch|swatches)$",
    )
    .unwrap()
});

/// Licence, readme and credits files: linked to their pack, never indexed for search content.
static META: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(licen[cs]e|readme|credits?|attribution|terms|eula|copying|changelog|version)[^/]*$")
        .unwrap()
});

/// Browser re-downloads get " (1)" before the extension.
static COPY_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" \(\d+\)(\.[^.]+)$").unwrap());

pub fn kind(ext: &str) -> Option<&'static str> {
    KIND.iter().find(|(_, exts)| exts.split_whitespace().any(|e| e == ext)).map(|(k, _)| *k)
}

/// Lower-case extension of a file name, "(none)" without one (a leading dot doesn't count).
pub fn ext_of(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    match base.get(1..).and_then(|rest| rest.rfind('.')) {
        Some(i) => base[i + 2..].to_lowercase(),
        None => "(none)".into(),
    }
}

/// Category of `rel` (a path relative to the library, zip entries as `<zip name>/<entry>`).
/// The same signature as the Python `classify()`, which the shared test cases call.
#[cfg(test)]
pub fn classify(rel: &str, ext: &str) -> &'static str {
    classify_why(rel, ext).0
}

/// Category plus the rule that decided it, shown in the detail panel ("why is this music?").
pub fn classify_why(rel: &str, ext: &str) -> (&'static str, String) {
    if JUNK.is_match(rel) {
        return ("junk", "junk: OS or tool leftover".into());
    }
    let Some(kind) = kind(ext) else {
        return ("other", format!("unknown extension .{ext}"));
    };
    if ext == "bvh" {
        return ("animation", ".bvh is always motion".into());
    }
    let low = rel.to_lowercase();
    let base = low.rsplit('/').next().unwrap_or(&low);
    if let Some((_, rules)) = WORDS.iter().find(|(k, _)| *k == kind) {
        for r in rules {
            let Some(m) = r.path.find(&low) else { continue };
            match &r.name {
                None => return (r.category, format!("{kind}: path word \"{}\"", m.as_str().trim_matches(['/', '_', '-', ' ', '.']))),
                Some(name) => {
                    if let Some(n) = name.find(base) {
                        return (r.category, format!("{kind}: file name \"{}\"", n.as_str()));
                    }
                }
            }
        }
    }
    let cat = DEFAULT_CAT.iter().find(|(k, _)| *k == kind).map(|(_, c)| *c).unwrap_or("other");
    (cat, format!("{kind}: .{ext}, no path word"))
}

pub fn is_meta(name: &str) -> bool {
    META.is_match(name)
}

/// The name a duplicate download shares with its original: "X (1).zip" -> "X.zip".
pub fn dup_name(name: &str) -> String {
    COPY_SUFFIX.replace(name, "$1").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// path -> expected category, the same 31 rows as test_survey_assets.py. Each row is a
    /// real mistake or a real naming pattern.
    const CASES: &[(&str, &str)] = &[
        ("FX/fire/Fire_01.png", "vfx"),
        ("Into the Breach/images/Vek_Firefly.png", "image"), // "fire" inside a word
        ("Pack/Animations/Sword_Slash.fbx", "animation"),    // animation folder
        ("Chars/Hero@Run.fbx", "animation"),                 // Mixamo/Unity clip name
        ("POLYGON_Fantasy/Animations/A_Walk_01.fbx", "animation"), // Synty clip prefix
        ("Guts/X_Presupport@hehestl/STL/Guts_baseA.stl", "print-model"), // "@" in a print folder
        ("Pack/Models/SM_Wall_01.fbx", "3d-model"),          // Synty static mesh
        ("Sounds/Music/Battle_Theme.ogg", "music"),
        ("Audio/VO/hero_hello.wav", "voice"),
        ("Audio/Ambience/forest_loop.wav", "ambience"),
        ("Audio/SFX/sword_hit.wav", "sound-effect"),
        ("x/song.aac", "music"), // .aac was missing at first
        // Zip entries are classified as "<zip name>/<entry>" (GameDev survey, 2026-10-04).
        ("Fantasy RPG Music Pack/Tracks/mp3/Action 1.mp3", "music"),
        ("Piano instrumental Vol. 2/Tracks/mp3/1. Echoes of Solitude.mp3", "music"), // "Vol" isn't "vo"
        ("Medieval Vol. 2/wav/Medieval Vol. 2 1.wav", "sound-effect"), // no music word: needs a probe
        ("Shooter Synthwave Music Pack/wav/(Soft Loop For Dialogues, Pause) 9.wav", "music"),
        ("Horror SFX/Ambient/Robots/Warning High Voltage.ogg", "ambience"),
        ("NOX_SOUND/Voices_Essentials/Voice_Female/Voice_Female_01.wav", "voice"),
        ("Shapeforms/Sci Fi Warp/Empty Void Suction Riser_02.wav", "sound-effect"),
        ("free-ambience-loops-audio/city-night-loop.wav", "ambience"),
        ("Free Fantasy SFX Pack By TomMusic/OGG Files/Sword_01.ogg", "sound-effect"), // "TomMusic"
        ("Shapeforms/__MACOSX/Shapeforms/._Cassette Preview", "junk"),
        ("Fantasy RPG Music Pack Vol.3/Tracks/mp3/desktop.ini", "junk"),
        ("POLYGON_Heist/Textures/.mayaSwatches/PolygonHeist_Texture_01_A.png_hcm.swatch", "junk"),
        ("female_body/Obj/obj.mtl", "material"),
        ("Textures/Rock_Normal.png", "texture"),
        ("UI/Icons/coin.png", "sprite"),
        ("mocap/walk.bvh", "animation"),
        ("Pack/Meshes/Barrel_01.obj", "3d-model"),
        ("Fonts/Roboto.ttf", "font"),
        ("Shaders/water.glsl", "shader"),
    ];

    #[test]
    fn same_answers_as_survey_assets_py() {
        assert_eq!(CASES.len(), 31);
        let mut bad = vec![];
        for (path, want) in CASES {
            // As the Python test: the extension is everything after the path's last dot.
            let ext = path.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
            let got = classify(path, &ext);
            if got != *want {
                bad.push(format!("{path}: got {got}, want {want}"));
            }
        }
        assert!(bad.is_empty(), "{} of {} failed:\n{}", bad.len(), CASES.len(), bad.join("\n"));
    }

    #[test]
    fn every_category_is_listed() {
        for (_, c) in DEFAULT_CAT {
            assert!(CATEGORIES.contains(c), "{c}");
        }
        for (_, rules) in WORDS.iter() {
            for r in rules {
                assert!(CATEGORIES.contains(&r.category), "{}", r.category);
            }
        }
    }

    #[test]
    fn extensions() {
        assert_eq!(ext_of("a/B.PNG"), "png");
        assert_eq!(ext_of("dir.v2/README"), "(none)");
        assert_eq!(ext_of(".DS_Store"), "(none)");
        assert_eq!(ext_of("x.tar.gz"), "gz");
    }

    #[test]
    fn duplicate_names() {
        assert_eq!(dup_name("POLYGON_Generic_SourceFiles_v3 (1).zip"), "POLYGON_Generic_SourceFiles_v3.zip");
        assert_eq!(dup_name("Track (2) live.wav"), "Track (2) live.wav");
    }

    #[test]
    fn why_names_the_rule() {
        let (cat, why) = classify_why("Fantasy RPG Music Pack/Tracks/mp3/Action 1.mp3", "mp3");
        assert_eq!(cat, "music");
        assert!(why.contains("music"), "{why}");
        assert!(is_meta("LICENSE.txt") && is_meta("Readme.pdf") && !is_meta("Rock.png"));
    }
}
