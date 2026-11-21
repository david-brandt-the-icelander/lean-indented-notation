//! `lin`: tools for LIN and JSON files, made for AI agents.
//!
//! Every command reads LIN (a file starting with `LIN `) or JSON. Commands that print data print
//! LIN by default, the cheaper form to read, and JSON with `--json`.

mod doc;
mod edit;
mod extra;
mod paths;
mod repo;
mod schemas;
mod views;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use lin::{json, Value, WriteOptions};

use doc::{Doc, Format, Layout, LoadOptions};
use schemas::Store;

pub type Res<T> = Result<T, String>;

pub fn fail<T>(msg: impl Into<String>) -> Res<T> {
    Err(msg.into())
}

#[derive(Parser)]
#[command(
    name = "lin",
    version,
    about = "Tools for LIN (Lean Indented Notation) and JSON files, made for AI agents.",
    long_about = "Tools for LIN (Lean Indented Notation) and JSON files, made for AI agents.\n\n\
Every command reads LIN or JSON (a LIN file starts with `LIN `). Commands that print data print LIN,\n\
the cheaper form to read, unless given --json. Paths look like plugins.0.title; quote keys that hold\n\
dots or spaces as JSON strings (files.\"a.json\"), or give a JSON Pointer (/plugins/0/title).\n\n\
Editing commands write the file back in its own format and also update its .lin/.json sibling, if it\n\
has one, so the two stay in step."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    /// Never fetch schemas over the network
    #[arg(long, global = true)]
    offline: bool,
    /// Read files that name a schema by the schema-less rules instead of fetching the schema
    #[arg(long, global = true)]
    no_schema: bool,
}

#[derive(clap::Args, Clone, Debug, Default)]
struct Out {
    /// Print JSON instead of LIN
    #[arg(short, long)]
    json: bool,
    /// With --json: one line, no spaces
    #[arg(short, long)]
    compact: bool,
    /// Print strings as raw text, not as LIN or JSON values
    #[arg(short, long)]
    raw: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print a LIN or JSON file as LIN (read JSON files this way to save tokens)
    Cat {
        file: Option<PathBuf>,
        #[command(flatten)]
        layout: Layout,
    },
    /// Convert LIN (or JSON) to JSON, 2-space indented
    ToJson {
        file: Option<PathBuf>,
        /// One line, no spaces
        #[arg(short, long)]
        compact: bool,
        /// Write to this file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Convert JSON (or LIN) to LIN
    FromJson {
        file: Option<PathBuf>,
        #[command(flatten)]
        layout: Layout,
        /// Name this schema in the header: a schema file (LIN or JSON) or a sha256- hash
        #[arg(long)]
        schema: Option<String>,
        /// The https:// location of the schema, written after its hash
        #[arg(long)]
        url: Option<String>,
        /// Write to this file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Rewrite LIN files in the writer's layout (keeps each file's header and form)
    Fmt {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[command(flatten)]
        layout: Layout,
        /// Change nothing; exit 1 if a file is not in the writer's layout
        #[arg(long)]
        check: bool,
    },
    /// Keys and nesting with list lengths, no values
    Outline {
        file: Option<PathBuf>,
        /// Stop at this depth
        #[arg(short, long)]
        depth: Option<usize>,
        /// Show the type of each scalar
        #[arg(short, long)]
        types: bool,
    },
    /// The head of a split-form file (structure and short values; long values are @)
    Head { file: Option<PathBuf> },
    /// One value: an object by its id, a value by reference name (owner.path), or a path
    Get {
        file: PathBuf,
        name: String,
        #[command(flatten)]
        out: Out,
    },
    /// One line per object that has an id: file, path, id, title or name
    Index {
        /// Files or directories [default: .]
        paths: Vec<PathBuf>,
    },
    /// Every scalar as one `path value` line (grep- and diff-friendly)
    Flat { file: Option<PathBuf> },
    /// Rebuild a document from `path value` lines (the output of lin flat)
    Unflat {
        file: Option<PathBuf>,
        #[command(flatten)]
        out: Out,
    },
    /// Search keys and values; prints `file:path value` for each match
    Grep {
        pattern: String,
        /// Files or directories [default: .]
        paths: Vec<PathBuf>,
        /// Ignore case
        #[arg(short, long)]
        ignore_case: bool,
        /// The pattern is plain text, not a regular expression
        #[arg(short = 'F', long)]
        fixed: bool,
        /// Match keys only
        #[arg(short, long, conflicts_with = "values")]
        keys: bool,
        /// Match values only
        #[arg(short = 'V', long)]
        values: bool,
        /// Print only the names of files with matches
        #[arg(short = 'l', long)]
        files_with_matches: bool,
        /// Print the number of matches per file
        #[arg(short = 'c', long)]
        count: bool,
    },
    /// Run a jq filter on LIN or JSON files (jq syntax, by jaq)
    #[command(visible_alias = "jq")]
    Query {
        filter: String,
        files: Vec<PathBuf>,
        #[command(flatten)]
        out: Out,
        /// Bind $NAME to a string
        #[arg(long, num_args = 2, value_names = ["NAME", "VALUE"])]
        arg: Vec<String>,
        /// Bind $NAME to a JSON value
        #[arg(long, num_args = 2, value_names = ["NAME", "JSON"])]
        argjson: Vec<String>,
        /// Read all inputs into one list
        #[arg(short, long)]
        slurp: bool,
        /// Use null as the input
        #[arg(short, long)]
        null_input: bool,
    },
    /// Set a value in a file (in place). VALUE is a LIN scalar: text, 12, true, ~, [], {} or a JSON string
    Set {
        file: PathBuf,
        path: String,
        value: String,
        /// VALUE is JSON (any value, objects and lists too)
        #[arg(long, conflicts_with = "string")]
        json: bool,
        /// VALUE is a string, whatever it looks like
        #[arg(long)]
        string: bool,
        /// Print the result instead of writing the file
        #[arg(long)]
        stdout: bool,
    },
    /// Delete values from a file (in place)
    Del {
        file: PathBuf,
        #[arg(required = true)]
        paths: Vec<String>,
        /// Print the result instead of writing the file
        #[arg(long)]
        stdout: bool,
    },
    /// Apply a JSON Patch (RFC 6902) or, with --merge, a merge patch (RFC 7386) to a file
    Patch {
        file: PathBuf,
        /// The patch, in LIN or JSON
        patch: PathBuf,
        #[arg(long)]
        merge: bool,
        /// Print the result instead of writing the file
        #[arg(long)]
        stdout: bool,
    },
    /// Deep-merge files, later ones winning, and print the result
    Merge {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Concatenate lists instead of replacing them
        #[arg(long)]
        concat: bool,
        #[command(flatten)]
        out: Out,
    },
    /// Show how two files' data differ, one line per change (+ added, - removed, ~ changed)
    Diff { a: PathBuf, b: PathBuf },
    /// The normalized SHA-256 of a schema (or any data), for a LIN header
    Hash { file: Option<PathBuf> },
    /// Check files: syntax, MUST rules, schemas, hashes, and .lin/.json siblings
    Check {
        /// Files or directories [default: .]
        paths: Vec<PathBuf>,
        /// Also require each file to be in the writer's layout
        #[arg(long)]
        layout: bool,
        /// Also require a .json beside each .lin file
        #[arg(long)]
        require_siblings: bool,
    },
    /// Validate a file against its schema (the header's, or --schema)
    Validate {
        file: PathBuf,
        /// A schema file to validate against instead of the header's
        #[arg(long)]
        schema: Option<PathBuf>,
    },
    /// Write the .json beside every .lin file (or the .lin from an edited .json), keeping them in step
    Sync {
        /// Files or directories [default: .]
        paths: Vec<PathBuf>,
        /// Change nothing; exit 1 if a pair is out of step or a .json is missing
        #[arg(long)]
        check: bool,
        /// Which file wins when both exist and differ
        #[arg(long, value_enum, default_value = "newer")]
        prefer: repo::Prefer,
    },
    /// Work with schemas and the content-addressed registries
    Schema {
        #[command(subcommand)]
        cmd: SchemaCmd,
    },
    /// A list of flat objects (a table) as CSV
    ToCsv {
        file: Option<PathBuf>,
        /// The path of the table in the file [default: the root]
        #[arg(short, long)]
        path: Option<String>,
    },
    /// CSV with a header row to a LIN table
    FromCsv {
        file: Option<PathBuf>,
        /// Keep every field as a string
        #[arg(long)]
        strings: bool,
        #[command(flatten)]
        out: Out,
    },
    /// Token counts (o200k_base) of files as JSON, minified JSON, LIN, split head and outline
    #[cfg(feature = "tokens")]
    Tokens {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SchemaCmd {
    /// Infer a JSON Schema from example files, written in LIN
    Infer {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[command(flatten)]
        out: Out,
    },
    /// Find a schema by hash (local stores, cache, registries) and print it
    Get {
        hash: String,
        #[command(flatten)]
        out: Out,
    },
    /// Copy a schema into a content-addressed directory as <hash>.lin and <hash>.json
    Publish {
        file: PathBuf,
        /// The store directory
        #[arg(long, default_value = "schemas")]
        dir: PathBuf,
    },
    /// Rewrite index.lin (and index.json) of a schema store: one row per schema
    Index {
        /// The store directory
        #[arg(long, default_value = "schemas")]
        dir: PathBuf,
    },
    /// List the places schemas are looked up, in order
    Where { file: Option<PathBuf> },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let store = Store::new(cli.offline);
    match run(cli, &store) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("lin: {e}");
            ExitCode::from(2)
        }
    }
}

fn print(s: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(s.as_bytes());
}

fn show(v: &Value, o: &Out) -> String {
    if o.raw {
        if let Value::String(s) = v {
            return format!("{s}\n");
        }
    }
    if o.json {
        if o.compact {
            format!("{}\n", json::to_string(v))
        } else {
            json::to_string_pretty(v)
        }
    } else {
        lin::to_fragment(v, &WriteOptions::default())
    }
}

fn whole(v: &Value, o: &Out) -> Res<String> {
    if o.json {
        Ok(show(v, o))
    } else if v.is_container() {
        lin::to_string(v).map_err(|e| e.to_string())
    } else {
        Ok(show(v, o))
    }
}

fn roots(p: Vec<PathBuf>) -> Vec<PathBuf> {
    if p.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        p
    }
}

fn parse_value_arg(s: &str, as_json: bool, as_string: bool) -> Res<Value> {
    if as_string {
        Ok(Value::String(s.to_string()))
    } else if as_json {
        json::parse(s).map_err(|e| e.to_string())
    } else {
        views::parse_scalar_arg(s)
    }
}

fn is_json_with_lin_sibling(f: &Path) -> bool {
    f.extension().is_some_and(|e| e == "json") && doc::sibling(f).is_some_and(|s| s.exists())
}

fn run(cli: Cli, store: &Store) -> Res<ExitCode> {
    let lo = LoadOptions {
        store,
        no_schema: cli.no_schema,
    };
    let load = |p: Option<&Path>| doc::load(p, &lo);
    match cli.cmd {
        Cmd::Cat { file, layout } => {
            let d = load(file.as_deref())?;
            let opts = layout.options(layout.split, &d.header);
            print(
                &lin::to_string_with(&d.value, &opts, d.schema.as_ref())
                    .map_err(|e| e.to_string())?,
            );
        }
        Cmd::ToJson {
            file,
            compact,
            output,
        } => {
            let d = load(file.as_deref())?;
            let s = if compact {
                format!("{}\n", json::to_string(&d.value))
            } else {
                json::to_string_pretty(&d.value)
            };
            match output {
                Some(o) => doc::write_file(&o, &s)?,
                None => print(&s),
            }
        }
        Cmd::FromJson {
            file,
            layout,
            schema,
            url,
            output,
        } => {
            let d = load(file.as_deref())?;
            let (hash, schema_doc) = match schema {
                None => (d.header.schema.clone(), d.schema.clone()),
                Some(s) if lin::jcs::is_hash(&s) => {
                    let sd = store.get(&s, url.as_deref(), file.as_deref()).ok();
                    (Some(s), sd)
                }
                Some(s) => {
                    let sd = load(Some(Path::new(&s)))?.value;
                    (Some(lin::schema_hash(&sd)), Some(sd))
                }
            };
            let header = lin::Header {
                version: 1,
                schema: hash,
                url: url.or(d.header.url.clone()),
            };
            let opts = layout.options(layout.split, &header);
            let s = lin::to_string_with(&d.value, &opts, schema_doc.as_ref())
                .map_err(|e| e.to_string())?;
            match output {
                Some(o) => doc::write_file(&o, &s)?,
                None => print(&s),
            }
        }
        Cmd::Fmt {
            files,
            layout,
            check,
        } => {
            let mut bad = 0;
            for f in files {
                let d = load(Some(&f))?;
                if d.format != Format::Lin {
                    return fail(format!(
                        "{}: not a LIN file (use lin from-json)",
                        f.display()
                    ));
                }
                let want = doc::render(&d, &d.value, &layout)?;
                let have = std::fs::read_to_string(&f).map_err(|e| e.to_string())?;
                if want != have {
                    if check {
                        println!("{}", f.display());
                        bad += 1;
                    } else {
                        doc::write_file(&f, &want)?;
                        eprintln!("formatted {}", f.display());
                    }
                }
            }
            return Ok(if bad > 0 {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            });
        }
        Cmd::Outline { file, depth, types } => {
            let d = load(file.as_deref())?;
            print(&views::outline(&d.value, depth, types));
        }
        Cmd::Head { file } => {
            let text = doc::read_text(file.as_deref())?;
            if doc::detect(&text) == Format::Lin {
                match text.split('\n').position(|l| l == "---") {
                    Some(n) => {
                        print(&(text.split('\n').take(n).collect::<Vec<_>>().join("\n") + "\n"))
                    }
                    None => print(&text),
                }
            } else {
                let d = doc::load_text(text, file.as_deref(), &lo)?;
                let opts = WriteOptions {
                    split: true,
                    ..Default::default()
                };
                let s = lin::to_string_with(&d.value, &opts, None).map_err(|e| e.to_string())?;
                print(
                    &(s.split("\n---\n")
                        .next()
                        .unwrap_or("")
                        .trim_end_matches('\n')
                        .to_string()
                        + "\n"),
                );
            }
        }
        Cmd::Get { file, name, out } => {
            let d = load(Some(&file))?;
            let (_, v) = paths::lookup(&d.value, &name)?;
            print(&show(&v, &out));
        }
        Cmd::Index { paths: ps } => {
            let ignore = repo::Ignore::load();
            let mut s = String::new();
            for f in repo::files(&roots(ps), &[".lin", ".json"], &ignore) {
                if is_json_with_lin_sibling(&f) {
                    continue; // the .lin already lists it
                }
                if let Ok(d) = load(Some(&f)) {
                    for (p, id, title) in views::index(&d.value) {
                        let c = |x: &str| lin::write::string_form(x, true, false).unwrap();
                        s.push_str(&format!(
                            " {}|{}|{}|{}\n",
                            c(&f.display().to_string()),
                            c(&p),
                            id,
                            if title.is_empty() {
                                "~".into()
                            } else {
                                c(&title)
                            }
                        ));
                    }
                }
            }
            if !s.is_empty() {
                print(&format!("[file|path|id|title]\n{s}"));
            }
        }
        Cmd::Flat { file } => {
            let d = load(file.as_deref())?;
            print(&views::flat(&d.value));
        }
        Cmd::Unflat { file, out } => {
            let text = doc::read_text(file.as_deref())?;
            print(&whole(&views::unflat(&text)?, &out)?);
        }
        Cmd::Grep {
            pattern,
            paths: ps,
            ignore_case,
            fixed,
            keys,
            values,
            files_with_matches,
            count,
        } => {
            let pat = if fixed {
                regex::escape(&pattern)
            } else {
                pattern
            };
            let re = regex::RegexBuilder::new(&pat)
                .case_insensitive(ignore_case)
                .build()
                .map_err(|e| e.to_string())?;
            let o = views::GrepOpts {
                re,
                keys_only: keys,
                values_only: values,
            };
            let ignore = repo::Ignore::load();
            let mut found = false;
            for f in repo::files(&roots(ps), &[".lin", ".json"], &ignore) {
                if is_json_with_lin_sibling(&f) {
                    continue; // same data as the .lin beside it
                }
                let d = match load(Some(&f)) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("lin: {e}");
                        continue;
                    }
                };
                let hits = views::grep(&d.value, &o);
                if hits.is_empty() {
                    continue;
                }
                found = true;
                if files_with_matches {
                    println!("{}", f.display());
                } else if count {
                    println!("{}:{}", f.display(), hits.len());
                } else {
                    let mut s = String::new();
                    for h in hits {
                        s.push_str(&format!("{}:{h}\n", f.display()));
                    }
                    print(&s);
                }
            }
            return Ok(if found {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            });
        }
        Cmd::Query {
            filter,
            files,
            out,
            arg,
            argjson,
            slurp,
            null_input,
        } => {
            let mut vars = Vec::new();
            for c in arg.chunks(2) {
                vars.push((c[0].clone(), Value::String(c[1].clone())));
            }
            for c in argjson.chunks(2) {
                vars.push((
                    c[0].clone(),
                    json::parse(&c[1]).map_err(|e| format!("--argjson {}: {e}", c[0]))?,
                ));
            }
            let q = extra::Query { filter, vars };
            let inputs: Vec<Value> = if null_input {
                vec![Value::Null]
            } else if files.is_empty() {
                vec![load(None)?.value]
            } else {
                files
                    .iter()
                    .map(|f| load(Some(f)).map(|d| d.value))
                    .collect::<Res<_>>()?
            };
            let inputs = if slurp {
                vec![Value::Array(inputs)]
            } else {
                inputs
            };
            let mut s = String::new();
            for i in &inputs {
                for v in extra::run_query(&q, i)? {
                    s.push_str(&show(&v, &out));
                }
            }
            print(&s);
        }
        Cmd::Set {
            file,
            path,
            value,
            json: as_json,
            string,
            stdout,
        } => {
            let d = load(Some(&file))?;
            let mut v = d.value.clone();
            let p = paths::parse(&path)?;
            paths::set(&mut v, &p, parse_value_arg(&value, as_json, string)?)?;
            finish_edit(&d, &v, stdout, store)?;
        }
        Cmd::Del {
            file,
            paths: ps,
            stdout,
        } => {
            let d = load(Some(&file))?;
            let mut v = d.value.clone();
            for p in ps {
                paths::remove(&mut v, &paths::parse(&p)?)?;
            }
            finish_edit(&d, &v, stdout, store)?;
        }
        Cmd::Patch {
            file,
            patch,
            merge,
            stdout,
        } => {
            let d = load(Some(&file))?;
            let p = load(Some(&patch))?.value;
            let mut v = d.value.clone();
            if merge {
                edit::merge_patch(&mut v, &p);
            } else {
                edit::json_patch(&mut v, &p)?;
            }
            finish_edit(&d, &v, stdout, store)?;
        }
        Cmd::Merge { files, concat, out } => {
            let mut it = files.iter();
            let mut v = load(Some(it.next().unwrap()))?.value;
            for f in it {
                edit::deep_merge(&mut v, &load(Some(f))?.value, concat);
            }
            print(&whole(&v, &out)?);
        }
        Cmd::Diff { a, b } => {
            let (x, y) = (load(Some(&a))?.value, load(Some(&b))?.value);
            let lines = views::diff(&x, &y);
            let mut s = String::new();
            for l in &lines {
                s.push_str(l);
                s.push('\n');
            }
            print(&s);
            return Ok(if lines.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            });
        }
        Cmd::Hash { file } => {
            let d = load(file.as_deref())?;
            println!("{}", lin::schema_hash(&d.value));
        }
        Cmd::Check {
            paths: ps,
            layout,
            require_siblings,
        } => {
            let ignore = repo::Ignore::load();
            let o = repo::CheckOptions {
                no_schema: cli.no_schema,
                layout,
                require_siblings,
            };
            let files = repo::files(&roots(ps), &[".lin", ".json"], &ignore);
            let mut errs = 0;
            for f in &files {
                for e in repo::check_file(f, store, &o) {
                    println!("{e}");
                    errs += 1;
                }
            }
            eprintln!("checked {} files: {} problems", files.len(), errs);
            return Ok(if errs > 0 {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            });
        }
        Cmd::Validate { file, schema } => {
            let d = load(Some(&file))?;
            let s = match schema {
                Some(p) => load(Some(&p))?.value,
                None => d
                    .schema
                    .clone()
                    .ok_or("the file names no schema; pass --schema")?,
            };
            let errs = extra::validate(&d.value, &s)?;
            for e in &errs {
                println!("{}: {e}", file.display());
            }
            return Ok(if errs.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            });
        }
        Cmd::Sync {
            paths: ps,
            check,
            prefer,
        } => {
            let ignore = repo::Ignore::load();
            let mut changed = 0;
            for f in repo::files(&roots(ps), &[".lin"], &ignore) {
                match repo::sync_file(&f, store, prefer, check) {
                    Ok(Some(msg)) => {
                        changed += 1;
                        if check {
                            let _ = msg;
                            println!(
                                "{}: out of step with its .json sibling (run lin sync)",
                                f.display()
                            );
                        } else {
                            eprintln!("{msg}");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        changed += 1;
                        println!("{e}");
                    }
                }
            }
            return Ok(if check && changed > 0 {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            });
        }
        Cmd::Schema { cmd } => match cmd {
            SchemaCmd::Infer { files, out } => {
                let docs: Vec<Value> = files
                    .iter()
                    .map(|f| load(Some(f)).map(|d| d.value))
                    .collect::<Res<_>>()?;
                let s = extra::infer(&docs);
                if out.json {
                    print(&show(&s, &out));
                } else {
                    print(&schema_text(&s, store)?);
                }
            }
            SchemaCmd::Get { hash, out } => {
                let v = store.get(&hash, None, None)?;
                if out.json {
                    print(&show(&v, &out));
                } else {
                    print(&schema_text(&v, store)?);
                }
            }
            SchemaCmd::Publish { file, dir } => {
                let d = load(Some(&file))?;
                let h = lin::schema_hash(&d.value);
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                doc::write_file(
                    &dir.join(format!("{h}.lin")),
                    &schema_text(&d.value, store)?,
                )?;
                doc::write_file(
                    &dir.join(format!("{h}.json")),
                    &json::to_string_pretty(&d.value),
                )?;
                write_index(&dir, store)?;
                println!("{h}");
            }
            SchemaCmd::Index { dir } => write_index(&dir, store)?,
            SchemaCmd::Where { file } => {
                for d in store.local_dirs(file.as_deref()) {
                    println!("{}", d.display());
                }
                println!(
                    "(built into lin: {} schemas)",
                    schemas::embedded_schemas().len()
                );
                println!("(the header's url, if it has one)");
                for (name, prefix) in schemas::registries() {
                    println!("{prefix}<hash>.lin  ({name})");
                }
            }
        },
        Cmd::ToCsv { file, path } => {
            let d = load(file.as_deref())?;
            let p = paths::parse(path.as_deref().unwrap_or("."))?;
            let v = paths::get(&d.value, &p).ok_or("nothing at that path")?;
            print(&extra::to_csv(v)?);
        }
        Cmd::FromCsv { file, strings, out } => {
            let text = doc::read_text(file.as_deref())?;
            print(&whole(&extra::from_csv(&text, strings)?, &out)?);
        }
        #[cfg(feature = "tokens")]
        Cmd::Tokens { files } => {
            let count = extra::token_counter()?;
            let mut s = String::from("[file|json|minified|lin|head|outline]\n");
            for f in files {
                let v = load(Some(&f))?.value;
                let plain = lin::to_string(&v).map_err(|e| e.to_string())?;
                let split = lin::to_string_with(
                    &v,
                    &WriteOptions {
                        split: true,
                        ..Default::default()
                    },
                    None,
                )
                .map_err(|e| e.to_string())?;
                let head = split.split("\n---\n").next().unwrap_or("");
                s.push_str(&format!(
                    " {}|{}|{}|{}|{}|{}\n",
                    lin::write::string_form(&f.display().to_string(), true, false).unwrap(),
                    count(&json::to_string_pretty(&v)),
                    count(&json::to_string(&v)),
                    count(&plain),
                    count(head),
                    count(&views::outline(&v, None, false))
                ));
            }
            print(&s);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// index.lin of a schema store: hash, $id and title of every schema in it.
fn write_index(dir: &Path, store: &Store) -> Res<()> {
    let mut rows = Vec::new();
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for p in names {
        let f = p.file_name().unwrap().to_string_lossy().to_string();
        if !(f.starts_with("sha256-") && f.ends_with(".json")) {
            continue;
        }
        let v = json::parse(&std::fs::read_to_string(&p).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let mut row = lin::Map::new();
        row.insert(
            "hash".into(),
            Value::String(f.trim_end_matches(".json").into()),
        );
        row.insert("id".into(), v.get("$id").cloned().unwrap_or(Value::Null));
        row.insert(
            "title".into(),
            v.get("title").cloned().unwrap_or(Value::Null),
        );
        rows.push(Value::Object(row));
    }
    let mut m = lin::Map::new();
    m.insert("schemas".into(), Value::Array(rows));
    let v = Value::Object(m);
    doc::write_file(
        &dir.join("index.lin"),
        &lin::to_string(&v).map_err(|e| e.to_string())?,
    )?;
    doc::write_file(&dir.join("index.json"), &json::to_string_pretty(&v))?;
    let _ = store;
    Ok(())
}

/// A schema written in LIN, its header naming the LIN meta-schema (JSON Schema 2020-12).
fn schema_text(s: &Value, store: &Store) -> Res<String> {
    let meta = store.get(extra::META, None, None).ok();
    let opts = WriteOptions {
        schema: meta.as_ref().map(|_| extra::META.to_string()),
        ..Default::default()
    };
    lin::to_string_with(s, &opts, meta.as_ref()).map_err(|e| e.to_string())
}

fn finish_edit(d: &Doc, v: &Value, stdout: bool, store: &Store) -> Res<()> {
    if let Some(schema) = &d.schema {
        for e in extra::validate(v, schema).unwrap_or_default() {
            eprintln!("lin: warning: the result does not match the file's schema: {e}");
        }
    }
    if stdout {
        print(&doc::render(d, v, &Layout::default())?);
    } else {
        for p in doc::save(d, v, &Layout::default(), store)? {
            eprintln!("wrote {}", p.display());
        }
    }
    Ok(())
}
