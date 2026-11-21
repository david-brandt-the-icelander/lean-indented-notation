//! Runs the shared conformance suite in tests/conformance (see its README).

use std::fs;
use std::path::{Path, PathBuf};

use lin::{json, schema_hash, ReadOptions, WriteOptions};

fn suite() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}

fn files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(suite().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(ext))
        .collect();
    v.sort();
    v
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap()
}

fn sibling_json(p: &Path) -> PathBuf {
    PathBuf::from(p.to_string_lossy().trim_end_matches(".lin").to_string() + ".json")
}

#[test]
fn write_cases() {
    let mut n = 0;
    for p in files("write", ".json") {
        let src = read(&p);
        let v = json::parse(&src).unwrap();
        assert_eq!(json::to_string_pretty(&v), src, "{p:?}: canonical JSON");
        let lin_path =
            PathBuf::from(p.to_string_lossy().trim_end_matches(".json").to_string() + ".lin");
        let want = read(&lin_path);
        let opts = WriteOptions {
            split: lin_path.to_string_lossy().ends_with(".split.lin"),
            ..Default::default()
        };
        let got = lin::to_string_with(&v, &opts, None).unwrap();
        assert_eq!(got, want, "{lin_path:?}: writer output");
        assert_eq!(lin::from_str(&want).unwrap(), v, "{lin_path:?}: reads back");
        n += 1;
    }
    assert!(n > 40);
}

#[test]
fn read_cases() {
    for p in files("read", ".lin") {
        let want = json::parse(&read(&sibling_json(&p))).unwrap();
        let got = lin::from_str(&read(&p)).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        assert_eq!(got, want, "{p:?}");
    }
}

#[test]
fn invalid_cases() {
    for p in files("invalid", ".lin") {
        let r = lin::from_str(&read(&p));
        assert!(r.is_err(), "{p:?} was accepted: {r:?}");
    }
}

#[test]
fn schema_cases() {
    let schemas = files("schemas", ".json");
    let schema = json::parse(&read(&schemas[0])).unwrap();
    let h = schema_hash(&schema);
    assert!(schemas[0].to_string_lossy().contains(&h));
    for p in files("schema", ".lin") {
        let text = read(&p);
        let want = json::parse(&read(&sibling_json(&p))).unwrap();
        assert_eq!(
            lin::from_str_with_schema(&text, Some(&schema), ReadOptions::default()).unwrap(),
            want,
            "{p:?}"
        );
        assert!(lin::from_str(&text).is_err(), "schema required");
        let split = p.to_string_lossy().ends_with(".split.lin");
        let opts = WriteOptions {
            split,
            split_limit: if split { 5 } else { 24 },
            schema: Some(h.clone()),
            ..Default::default()
        };
        assert_eq!(
            lin::to_string_with(&want, &opts, Some(&schema)).unwrap(),
            text,
            "{p:?}: writer"
        );
    }
}

#[test]
fn hashes_of_published_schemas() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("sha256-") && name.ends_with(".json") {
            let v = json::parse(&read(&p)).unwrap();
            assert_eq!(format!("{}.json", schema_hash(&v)), name);
        }
    }
}
