//! PyO3 binding: `rowan_rs.analyse(source, filename, search_paths, entry_points, follow_imports,
//! use_summaries, row_checks)` returns the analysis as a JSON string (the same document the
//! CLI's `--golden` mode prints), so `tests/run_golden.py` can run both engines in-process.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use ::rowan_rs::interp::Analyzer;
use ::rowan_rs::report;

#[pyfunction]
#[pyo3(signature = (source, filename="<module>", search_paths=vec![], entry_points=true, follow_imports=true, use_summaries=None, row_checks=vec![], compat=false))]
#[allow(clippy::too_many_arguments)]
fn analyse(py: Python<'_>, source: String, filename: &str, search_paths: Vec<String>, entry_points: bool, follow_imports: bool, use_summaries: Option<bool>, row_checks: Vec<(u32, String)>, compat: bool) -> PyResult<String> {
    let filename = filename.to_string();
    let result = py.allow_threads(move || {
        std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || -> Result<String, String> {
                let mut an = Analyzer::new(source, &filename, search_paths, entry_points, follow_imports, use_summaries)?;
                if compat {
                    an.opts = ::rowan_rs::interp::Options::compat();
                }
                an.row_check_requests = row_checks;
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
    analyse(py, src, &path, vec![dir], entry_points, follow_imports, None, vec![], false)
}

#[pymodule]
#[pyo3(name = "rowan_rs")]
fn rowan_rs_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(analyse, m)?)?;
    m.add_function(wrap_pyfunction!(analyse_file, m)?)?;
    Ok(())
}
