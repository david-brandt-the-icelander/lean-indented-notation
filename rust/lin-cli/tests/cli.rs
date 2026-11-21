//! End-to-end tests of the lin command.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lin(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_lin"))
        .args(args)
        .current_dir(dir)
        .env("LIN_CACHE_DIR", dir.join(".cache"))
        .env("LIN_REGISTRIES", "")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lin-cli-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("schemas")).unwrap();
    for f in [
        "plugin-registry.lin",
        "plugin-registry.json",
        "plugin-registry.split.lin",
    ] {
        fs::copy(repo().join("examples").join(f), d.join(f)).unwrap();
    }
    for e in fs::read_dir(repo().join("examples/schemas")).unwrap() {
        let p = e.unwrap().path();
        fs::copy(&p, d.join("schemas").join(p.file_name().unwrap())).unwrap();
    }
    d
}

#[test]
fn convert_both_ways() {
    let d = scratch("convert");
    let (c, j, _) = lin(&d, &["to-json", "plugin-registry.lin"]);
    assert_eq!(c, 0);
    assert_eq!(
        j,
        fs::read_to_string(d.join("plugin-registry.json")).unwrap()
    );
    let (c, l, _) = lin(
        &d,
        &[
            "from-json",
            "plugin-registry.json",
            "--schema",
            "schemas/sha256-M804eExtUC7S2al_N82_CXtgd3ip0TT4ppYaSQtyHos.lin",
            "--url",
            "https://schemas.example.org/plugin-registry.lin",
        ],
    );
    assert_eq!(c, 0);
    assert_eq!(
        l,
        fs::read_to_string(d.join("plugin-registry.lin")).unwrap()
    );
    let (_, l, _) = lin(
        &d,
        &[
            "fmt",
            "--check",
            "plugin-registry.lin",
            "plugin-registry.split.lin",
        ],
    );
    assert_eq!(l, "");
    let (_, s, _) = lin(&d, &["cat", "--split", "plugin-registry.json"]);
    assert!(s.contains("\n---\n@description "));
}

#[test]
fn views() {
    let d = scratch("views");
    let (_, o, _) = lin(&d, &["outline", "plugin-registry.lin"]);
    assert!(o.starts_with("name\nversion\ndescription\nplugins[2]\n id\n"));
    let (_, g, _) = lin(
        &d,
        &["get", "plugin-registry.lin", "spell-check.summary", "-r"],
    );
    assert_eq!(
        g,
        "Underlines misspelt words as you type and offers corrections.\n"
    );
    let (_, g, _) = lin(&d, &["get", "plugin-registry.lin", "/plugins/1/priority"]);
    assert_eq!(g, "\"20\"\n");
    let (_, h, _) = lin(&d, &["head", "plugin-registry.split.lin"]);
    assert!(h.ends_with(" tags []\n") && !h.contains("---"));
    let (c, g, _) = lin(&d, &["grep", "-k", "^lines$", "."]);
    assert_eq!(c, 0);
    assert!(g.contains("plugin-registry.lin:plugins.0.sources.0.lines 120"));
    let (_, i, _) = lin(&d, &["index", "plugin-registry.lin"]);
    assert_eq!(i, "[file|path|id|title]\n plugin-registry.lin|plugins.0|spell-check|Spell check\n plugin-registry.lin|plugins.1|word-count|Word count\n");
    let (_, f, _) = lin(&d, &["flat", "plugin-registry.lin"]);
    assert!(f.contains("plugins.1.priority \"20\"\n"));
    fs::write(d.join("flat.txt"), &f).unwrap();
    let (_, u, _) = lin(&d, &["unflat", "flat.txt", "--json"]);
    // without the schema, flat text reads back the same data
    assert_eq!(
        u,
        fs::read_to_string(d.join("plugin-registry.json")).unwrap()
    );
}

#[test]
fn query() {
    let d = scratch("query");
    let (c, q, e) = lin(
        &d,
        &[
            "query",
            "[.plugins[] | select(.enabled) | .id]",
            "plugin-registry.lin",
            "-j",
            "-c",
        ],
    );
    assert_eq!(c, 0, "{e}");
    assert_eq!(q, "[\"spell-check\"]\n");
    let (_, q, _) = lin(
        &d,
        &[
            "jq",
            "-r",
            "--arg",
            "x",
            "word-count",
            ".plugins[] | select(.id == $x) | .title",
            "plugin-registry.lin",
        ],
    );
    assert_eq!(q, "Word count\n");
    let (_, q, _) = lin(
        &d,
        &["jq", ".plugins | map({id, tags})", "plugin-registry.json"],
    );
    assert_eq!(
        q,
        "+ id spell-check\n tags[] text|language\n+ id word-count\n tags []\n"
    );
}

#[test]
fn edits_keep_siblings_in_step() {
    let d = scratch("edits");
    let (c, _, e) = lin(
        &d,
        &["set", "plugin-registry.lin", "plugins.0.priority", "5"],
    );
    assert_eq!(c, 0, "{e}");
    assert!(fs::read_to_string(d.join("plugin-registry.json"))
        .unwrap()
        .contains("\"priority\": 5,"));
    let (c, _, _) = lin(
        &d,
        &["set", "plugin-registry.json", "plugins.0.title", "Spelling"],
    );
    assert_eq!(c, 0);
    assert!(fs::read_to_string(d.join("plugin-registry.lin"))
        .unwrap()
        .contains(" title Spelling\n"));
    let (_, _, e) = lin(
        &d,
        &["set", "plugin-registry.lin", "plugins.0.enabled", "maybe"],
    );
    assert!(e.contains("warning"));
    let (c, out, _) = lin(&d, &["check", "plugin-registry.lin"]);
    assert_eq!(c, 1);
    assert!(out.contains("plugins.0.enabled"));
    lin(&d, &["del", "plugin-registry.lin", "plugins.0.enabled"]);
    let (c, _, _) = lin(&d, &["sync", "--check", "."]);
    assert_eq!(c, 1, "the split file has no .json sibling yet");
    lin(&d, &["sync", "."]);
    let (c, _, _) = lin(&d, &["sync", "--check", "."]);
    assert_eq!(c, 0);
}

#[test]
fn patch_merge_diff() {
    let d = scratch("patch");
    fs::write(d.join("p.json"), r#"[{"op":"replace","path":"/version","value":"3.0.0"},{"op":"remove","path":"/plugins/1"}]"#).unwrap();
    let (c, out, e) = lin(&d, &["patch", "plugin-registry.lin", "p.json", "--stdout"]);
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("version 3.0.0\n") && !out.contains("word-count"));
    fs::write(d.join("m.lin"), "LIN 1\nversion ~\nextra 1\n").unwrap();
    let (_, out, _) = lin(
        &d,
        &[
            "patch",
            "plugin-registry.lin",
            "m.lin",
            "--merge",
            "--stdout",
        ],
    );
    assert!(!out.contains("version") && out.contains("extra 1"));
    fs::write(d.join("b.lin"), "LIN 1\nname x\nplugins[id]\n a\n b\n").unwrap();
    fs::write(d.join("c.lin"), "LIN 1\nname y\nplugins[id]\n a\n z\n b\n").unwrap();
    let (c, out, _) = lin(&d, &["diff", "b.lin", "c.lin"]);
    assert_eq!(c, 1);
    assert_eq!(out, "~ name x -> y\n+ plugins.1 {\"id\":\"z\"}\n");
    let (_, out, _) = lin(&d, &["merge", "b.lin", "c.lin", "--concat"]);
    assert_eq!(out, "LIN 1\nname y\nplugins[id]\n a\n b\n a\n z\n b\n");
}

#[test]
fn schemas_and_hashes() {
    let d = scratch("schemas");
    let (_, h, _) = lin(
        &d,
        &[
            "hash",
            "schemas/sha256-M804eExtUC7S2al_N82_CXtgd3ip0TT4ppYaSQtyHos.json",
        ],
    );
    assert_eq!(
        h.trim(),
        "sha256-M804eExtUC7S2al_N82_CXtgd3ip0TT4ppYaSQtyHos"
    );
    let (c, out, e) = lin(&d, &["check", "."]);
    assert_eq!(c, 0, "{out}{e}");
    let (_, s, _) = lin(&d, &["schema", "infer", "plugin-registry.json"]);
    assert!(s.starts_with("LIN 1 sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars\n$schema "));
    fs::write(d.join("inferred.lin"), &s).unwrap();
    let (c, h, _) = lin(&d, &["schema", "publish", "inferred.lin", "--dir", "store"]);
    assert_eq!(c, 0);
    assert!(d.join("store").join(format!("{}.lin", h.trim())).exists());
    assert!(d.join("store/index.lin").exists());
    let (c, _, e) = lin(&d, &["--offline", "to-json", "plugin-registry.lin"]);
    assert_eq!(c, 0, "{e}");
    fs::remove_dir_all(d.join("schemas")).unwrap();
    let (c, _, e) = lin(&d, &["--offline", "to-json", "plugin-registry.lin"]);
    assert_eq!(c, 2);
    assert!(e.contains("could not be found"));
    let (c, _, _) = lin(&d, &["--no-schema", "to-json", "plugin-registry.lin"]);
    assert_eq!(c, 0);
    // the meta-schema is built in
    let (c, out, e) = lin(
        &d,
        &[
            "--offline",
            "schema",
            "get",
            "sha256-wcrPgr6mZdpNuvWOs0G3rzlCDUySSKASq6NDbvH4Ars",
        ],
    );
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("$dynamicAnchor meta"));
}

#[test]
fn csv_round_trip() {
    let d = scratch("csv");
    fs::write(d.join("t.csv"), "a,b,c\n1,\"x, y\",true\n2,,\"q\"\"\"\n").unwrap();
    let (_, out, _) = lin(&d, &["from-csv", "t.csv"]);
    assert_eq!(out, "LIN 1\n[a|b|c]\n 1|x, y|true\n 2|~|q\"\n");
    fs::write(d.join("t.lin"), &out).unwrap();
    let (_, back, _) = lin(&d, &["to-csv", "t.lin"]);
    assert_eq!(back, "a,b,c\n1,\"x, y\",true\n2,,\"q\"\"\"\n");
}

#[test]
fn repository_is_in_step() {
    let (c, out, e) = lin(&repo(), &["--offline", "check", "."]);
    assert_eq!(c, 0, "{out}{e}");
    let (c, out, _) = lin(&repo(), &["--offline", "sync", "--check", "."]);
    assert_eq!(c, 0, "{out}");
}
