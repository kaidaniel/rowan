//! CLI: rowan FILE.py [--json] [--golden] [--facts] [--no-rows] [--no-summaries] [--no-entry] [--no-imports]
use std::io::Write;

use rowan::interp::Analyzer;
use rowan::report;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file: Option<String> = None;
    let mut json = false;
    let mut golden = false;
    let mut facts = false;
    let mut show_rows = true;
    let mut show_summaries = true;
    let mut entry = true;
    let mut imports = true;
    for a in &args {
        match a.as_str() {
            "--json" => json = true,
            "--golden" => golden = true,
            "--facts" => facts = true,
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
        eprintln!("usage: rowan FILE.py [--json|--golden|--facts] [--no-rows] [--no-summaries] [--no-entry] [--no-imports]");
        std::process::exit(2);
    };
    let out = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || run(&file, json, golden, facts, show_rows, show_summaries, entry, imports))
        .unwrap()
        .join();
    match out {
        Ok(Ok(text)) => {
            let mut so = std::io::stdout().lock();
            let _ = so.write_all(text.as_bytes());
            let _ = so.write_all(b"\n");
        }
        Ok(Err(e)) => {
            eprintln!("rowan: {}", e);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("rowan: analysis panicked");
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run(file: &str, json: bool, golden: bool, facts: bool, show_rows: bool, show_summaries: bool, entry: bool, imports: bool) -> Result<String, String> {
    let src = std::fs::read_to_string(file).map_err(|e| format!("{}: {}", file, e))?;
    let dir = std::path::Path::new(file).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
    let dir = std::fs::canonicalize(&dir).map(|p| p.to_string_lossy().to_string()).unwrap_or(dir);
    let mut an = Analyzer::new(src.clone(), file, vec![dir], entry, imports, None)?;
    an.facts_on = facts;
    if golden {
        // `#@row NAME:` expectations are evaluated at the statement on that line
        for (i, line) in src.lines().enumerate() {
            // (`#@todo row NAME:` too: the harness checks those with ROWAN_TODO=check)
            let hit = line.find("#@row").map(|p| p + 5).or_else(|| line.find("#@todo row").map(|p| p + 10));
            if let Some(pos) = hit {
                let rest = &line[pos..];
                let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if !name.is_empty() {
                    an.row_check_requests.push((i as u32 + 1, name));
                }
            }
        }
    }
    an.run();
    if std::env::var("ROWAN_DEBUG").is_ok() {
        eprintln!("work: {} (tabulations {}, objects {}, {} ms{})", an.work, an.tab_count.values().sum::<usize>(), an.objects.len(), an.started.elapsed().as_millis(), if an.over_deadline() { ", degraded after the deadline" } else { "" });
    }
    if facts {
        return Ok(serde_json::to_string(&report::facts_json(&an)).unwrap());
    }
    if golden {
        return Ok(serde_json::to_string(&report::golden_json(&an)).unwrap());
    }
    if json {
        let lines: Vec<String> = report::to_json(&an).into_iter().map(|j| serde_json::to_string(&j).unwrap()).collect();
        return Ok(lines.join("\n"));
    }
    Ok(report::format_text(&an, show_rows, show_summaries))
}
