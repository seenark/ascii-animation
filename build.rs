use std::env;
use std::fs;
use std::path::PathBuf;

fn normalize_figlet_content(bytes: &[u8]) -> String {
    let content = match String::from_utf8(bytes.to_vec()) {
        Ok(content) => content,
        Err(_) => bytes.iter().copied().map(char::from).collect(),
    };
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let figlet_dir = manifest_dir.join("figlet");
    println!("cargo:rerun-if-changed=figlet");

    let entries = fs::read_dir(&figlet_dir)
        .unwrap_or_else(|err| panic!("failed to read figlet/: {err}"));

    let mut fonts = Vec::new();
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read figlet/ entry: {err}"));
        let path = entry.path();
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| panic!("failed to inspect figlet/ entry {}: {err}", path.display()));
        if !file_type.is_file() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("flf") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_else(|| panic!("figlet/ contains a non-UTF-8 .flf stem: {}", path.display()))
            .to_string();
        let content = fs::read(&path)
            .unwrap_or_else(|err| panic!("failed to read figlet/ font {}: {err}", path.display()));
        fonts.push((name, path, content));
    }

    if fonts.is_empty() {
        panic!("figlet/ must contain at least one .flf font file");
    }

    fonts.sort_by(|(left, _, _), (right, _, _)| {
        left.to_ascii_lowercase()
            .cmp(&right.to_ascii_lowercase())
            .then_with(|| left.cmp(right))
    });

    let mut generated = String::from("pub const FIGLET_FONTS: &[(&str, &str)] = &[\n");
    for (name, path, content) in &fonts {
        let relative = path
            .strip_prefix(&manifest_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        println!("cargo:rerun-if-changed={}", relative);
        generated.push_str(&format!(
            "    ({:?}, {:?}),\n",
            name,
            normalize_figlet_content(content)
        ));
    }
    generated.push_str("];\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("figlet_fonts.rs"), generated)
        .unwrap_or_else(|err| panic!("failed to write generated figlet/ manifest: {err}"));
}
