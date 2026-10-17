//! PyO3 binding: `rowan.analyse(source, filename, search_paths, entry_points, follow_imports,
//! row_checks)` returns the analysis as a JSON string (the same document the
//! CLI's `--golden` mode prints), so the engine can be driven from Python without the CLI (the wheel builds; loading it fails on macOS 26, see AGENTS.md).

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use ::rowan::interp::Analyzer;
use ::rowan::report;

#[pyfunction]
#[pyo3(signature = (source, filename="<module>", search_paths=vec![], entry_points=true, follow_imports=true, row_checks=vec![]))]
#[allow(clippy::too_many_arguments)]
fn analyse(py: Python<'_>, source: String, filename: &str, search_paths: Vec<String>, entry_points: bool, follow_imports: bool, row_checks: Vec<(u32, String)>) -> PyResult<String> {
    let filename = filename.to_string();
    let result = py.allow_threads(move || {
        std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || -> Result<String, String> {
                let mut an = Analyzer::new(source, &filename, search_paths, entry_points, follow_imports)?;
                for (line, name) in row_checks {
                    an.request_row_check((String::new(), line, name));
                }
                an.run();
                Ok(serde_json::to_string(&report::golden_json(&an)).unwrap())
            })
            .map_err(|e| e.to_string())?
            .join()
            .map_err(|_| "analysis panicked".to_string())?
    });
    result.map_err(PyRuntimeError::new_err)
}

#[pyfunction]
#[pyo3(signature = (path, entry_points=true, follow_imports=true))]
fn analyse_file(py: Python<'_>, path: String, entry_points: bool, follow_imports: bool) -> PyResult<String> {
    let src = std::fs::read_to_string(&path).map_err(|e| PyRuntimeError::new_err(format!("{}: {}", path, e)))?;
    let dir = std::path::Path::new(&path).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
    let dir = std::fs::canonicalize(&dir).map(|p| p.to_string_lossy().to_string()).unwrap_or(dir);
    analyse(py, src, &path, vec![dir], entry_points, follow_imports, vec![])
}

#[pymodule]
#[pyo3(name = "rowan")]
fn rowan_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(analyse, m)?)?;
    m.add_function(wrap_pyfunction!(analyse_file, m)?)?;
    Ok(())
}
