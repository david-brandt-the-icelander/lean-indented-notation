// Embeds the published schemas (../../schemas/<provisional id>.lin), the registry's index
// (../../schemas/index.lin) and the registry list (../../registries.lin), so the tool works
// offline for the LIN meta-schema and knows every registered number at build time.
use std::{env, fs, path::Path};

fn main() {
    // In the repository the files are two levels up; a packaged crate carries copies in embedded/
    // (the release script puts them there before `cargo publish`).
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = if here.join("embedded/schemas").is_dir() {
        here.join("embedded")
    } else {
        here.join("../..")
    };
    let dir = root.join("schemas");
    println!("cargo:rerun-if-changed={}", dir.display());
    println!(
        "cargo:rerun-if-changed={}",
        root.join("registries.lin").display()
    );
    let mut entries = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.len() == 43
                && name.ends_with(".lin")
                && name[..39].bytes().all(|c| c.is_ascii_digit())
            {
                let p = fs::canonicalize(e.path()).unwrap();
                println!("cargo:rerun-if-changed={}", p.display());
                entries.push(format!(
                    "({:?}, include_str!({:?})),",
                    name.trim_end_matches(".lin"),
                    p.display().to_string()
                ));
            }
        }
    }
    entries.sort();
    let reg = root.join("registries.lin");
    let reg_src = if reg.exists() {
        format!(
            "include_str!({:?})",
            fs::canonicalize(&reg).unwrap().display().to_string()
        )
    } else {
        "\"\"".to_string()
    };
    let idx = dir.join("index.lin");
    println!("cargo:rerun-if-changed={}", idx.display());
    let idx_src = if idx.exists() {
        format!(
            "include_str!({:?})",
            fs::canonicalize(&idx).unwrap().display().to_string()
        )
    } else {
        "\"\"".to_string()
    };
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("embedded.rs");
    fs::write(
        out,
        format!(
            "pub static SCHEMAS: &[(&str, &str)] = &[\n{}\n];\npub static REGISTRIES: &str = {};\npub static INDEX: &str = {};\n",
            entries.join("\n"),
            reg_src,
            idx_src
        ),
    )
    .unwrap();
}
