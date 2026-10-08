//! CLI: rowan-rs FILE.py [--json] [--golden] [--no-rows] [--no-summaries] [--no-entry] [--no-imports]
use std::io::Write;

use rowan_rs::interp::Analyzer;
use rowan_rs::report;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file: Option<String> = None;
    let mut json = false;
    let mut golden = false;
    let mut show_rows = true;
    let mut show_summaries = true;
    let mut entry = true;
    let mut imports = true;
    let mut compat = false;
    for a in &args {
        match a.as_str() {
            "--compat" => compat = true,
            "--json" => json = true,
            "--golden" => golden = true,
            "--no-rows" => show_rows = false,
            "--no-summaries" => show_summaries = false,
            "--no-entry" => entry = false,
            "--no-imports" => imports = false,
            other if other.starts_with("--") => {
                eprintln!("unknown option {}", other);
                std::process::exit(2);
            }
            other => file = Some(other.to_string()),
        }
    }
    let Some(file) = file else {
        eprintln!("usage: rowan-rs FILE.py [--json|--golden] [--no-rows] [--no-summaries] [--no-entry] [--no-imports] [--compat]");
        std::process::exit(2);
    };
    let out = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || run(&file, json, golden, show_rows, show_summaries, entry, imports, compat))
        .unwrap()
        .join();
    match out {
        Ok(Ok(text)) => {
            let mut so = std::io::stdout().lock();
            let _ = so.write_all(text.as_bytes());
            let _ = so.write_all(b"\n");
        }
        Ok(Err(e)) => {
            eprintln!("rowan-rs: {}", e);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("rowan-rs: analysis panicked");
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run(file: &str, json: bool, golden: bool, show_rows: bool, show_summaries: bool, entry: bool, imports: bool, compat: bool) -> Result<String, String> {
    let src = std::fs::read_to_string(file).map_err(|e| format!("{}: {}", file, e))?;
    let dir = std::path::Path::new(file).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
    let dir = std::fs::canonicalize(&dir).map(|p| p.to_string_lossy().to_string()).unwrap_or(dir);
    let mut an = Analyzer::new(src.clone(), file, vec![dir], entry, imports, None)?;
    if compat {
        an.opts = rowan_rs::interp::Options::compat();
    }
    if golden {
        // `#@row NAME:` expectations are evaluated at the statement on that line
        for (i, line) in src.lines().enumerate() {
            if let Some(pos) = line.find("#@row") {
                let rest = &line[pos + 5..];
                let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if !name.is_empty() {
                    an.row_check_requests.push((i as u32 + 1, name));
                }
            }
        }
    }
    an.run();
    if golden {
        return Ok(serde_json::to_string(&report::golden_json(&an)).unwrap());
    }
    if json {
        let lines: Vec<String> = report::to_json(&an).into_iter().map(|j| serde_json::to_string(&j).unwrap()).collect();
        return Ok(lines.join("\n"));
    }
    Ok(report::format_text(&an, show_rows, show_summaries))
}
