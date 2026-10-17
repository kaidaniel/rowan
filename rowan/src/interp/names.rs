//! The report's names for what rowan cannot see: unknown objects named in the program's own
//! terms (`a`, `a(b)`, `cfg["db"]`, `rows[*]`, `json.loads(text)`, printed instead of `?`),
//! where unknown results came from, and unknown call sites as written (sources and mutators
//! are named by them). Write-once side tables for the text report, the JSON and the hovers;
//! no verdict depends on them, but the torch model reads them back (names for dims,
//! `x.shape[0]` as `B`; whether an unknown is a call's result).

use rustc_hash::{FxHashMap, FxHashSet};

use super::Heap;
use crate::domain::{Ns, ObjId};
use crate::intern::{self, KeyId, StrId};
use crate::state::Kind;

#[derive(Default)]
pub struct Names {
    /// unknown objects named in the program's own terms
    symbols: FxHashMap<ObjId, String>,
    /// the function whose names a symbol uses (0: module level); it is printed only there
    scopes: FxHashMap<ObjId, u32>,
    /// results of unknown calls: (the call as written, its line, what it was given — which
    /// unknown code may return)
    pub origins: FxHashMap<ObjId, (String, u32, Vec<String>)>,
    /// unknown call sites (sources and mutators are named by them) as written: (the call's
    /// text, its module, its line), for reports
    pub site_names: FxHashMap<StrId, (String, u32, u32)>,
    /// the sites of calls of the program's code that did not run (`skipped_call`)
    pub skipped_sites: FxHashSet<StrId>,
}

impl Names {
    pub fn set(&mut self, oid: ObjId, sym: String, scope: u32) {
        self.symbols.insert(oid, sym);
        self.scopes.insert(oid, scope);
    }

    pub fn has(&self, oid: ObjId) -> bool {
        self.symbols.contains_key(&oid)
    }

    /// The object's symbol, wherever it is printed.
    pub fn get(&self, oid: ObjId) -> Option<&String> {
        self.symbols.get(&oid)
    }

    /// The function whose names the object's symbol uses (0: module level).
    pub fn scope(&self, oid: ObjId) -> u32 {
        self.scopes.get(&oid).copied().unwrap_or(0)
    }

    /// The symbol an object is printed by where the printer is now (`report::in_scope`): a
    /// symbol uses the names of its function, module-level ones are visible everywhere.
    pub fn visible(&self, oid: ObjId) -> Option<&String> {
        let s = self.scope(oid);
        if s == 0 || s == crate::interp::render::render_scope() {
            self.symbols.get(&oid)
        } else {
            None
        }
    }

    /// The symbol of `parent`'s content under (ns, key): `d["k"]`, `o.x`, `rows[*]`, a cell's
    /// variable; None when the parent has no symbol.
    pub fn access(&self, heap: &Heap, parent: ObjId, ns: Ns, key: KeyId) -> Option<String> {
        if heap.kind(parent) == Kind::Cell {
            return heap.obj(parent).label.strip_prefix("cell ").map(|x| x.to_string());
        }
        let base = self.symbols.get(&parent)?;
        Some(match ns {
            Ns::Attr => format!("{}.{}", base, intern::key_py_str(key)),
            Ns::Cell => base.clone(),
            _ if key == intern::STAR => format!("{}[*]", base),
            _ => format!("{}[{}]", base, crate::domain::key_display(key)),
        })
    }
}
