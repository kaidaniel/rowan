"""Tensor-shape expectations for the golden harness (`tests/torch/`).

    h = self.fc(x)        #@shape h: [*R, 10] where R=x.shape[:-1]
    n = x.size(0)         #@dim n: x.shape[0]
    y = m(x)              #@shape@L40 y: [32, 10]          (only the runs called from L40)
    z = x @ w             #@requires x.shape[-1] == 512
    q = lin(torch.ones(3, 4))   #@shape-error

Shapes: `[d, …]` with at most one rest `*NAME` (`*` = unknown leading/middle dims, `*_` = any
rest); dims are integer expressions compared as polynomials (`4*C` == `C*4`, `T*C` after
`view(B, -1)`); `?` is an unknown dim (the engine must say unknown), `_` accepts anything.
`[…] if C1, C2` is a shape that holds under assumptions (`x @ y` with ranks unknown: `if
x.dim() >= 2, y.dim() >= 2`); a shape without `if` must carry no assumptions.
`#@shape NAME: ?` accepts a value with no known shape (not a tensor, or `[*]`); `none` demands
that NAME is not a tensor.  `where A=…, B=…` substitutes names before comparing.
Without `@L`, a check applies to the runs in the function's own (entry/symbolic) analysis if it
has one, else to every run; each selected run must match.
A line holding only a directive comment applies to the code line above it.
`#@requires C1; C2` lists exactly the requirements recorded at that line (`none`: no
requirement).  `#@shape-error [text]`: a definite mismatch at that line (whose message contains
text); a shape error on a line without one fails the case.
"""
from __future__ import annotations

import ast
import re
from typing import Any, Dict, List, Optional, Tuple

Q = "__q__"  # `?`


def _split_top(s: str, sep: str = ",") -> List[str]:
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == sep and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur)
    return [p.strip() for p in out]


class _Subst(ast.NodeTransformer):
    def __init__(self, env: Dict[str, ast.AST]):
        self.env = env

    def visit_Name(self, node: ast.Name) -> ast.AST:
        if node.id in self.env:
            return self.env[node.id]
        return node


def _parse(expr: str, where: Dict[str, str]) -> ast.AST:
    expr = expr.replace("?", Q)
    tree = ast.parse(expr, mode="eval").body
    if where:
        env = {k: ast.parse(v.replace("?", Q), mode="eval").body for k, v in where.items()}
        # substitute until fixpoint (a where-name may use another)
        for _ in range(4):
            tree = _Subst(env).visit(tree)
    return tree


Poly = Dict[Tuple[Tuple[str, int], ...], int]


def _padd(a: Poly, b: Poly, k: int = 1) -> Poly:
    r = dict(a)
    for m, c in b.items():
        r[m] = r.get(m, 0) + k * c
        if r[m] == 0:
            del r[m]
    return r


def _pmul(a: Poly, b: Poly) -> Poly:
    r: Poly = {}
    for m1, c1 in a.items():
        for m2, c2 in b.items():
            d: Dict[str, int] = dict(m1)
            for at, p in m2:
                d[at] = d.get(at, 0) + p
            m = tuple(sorted(d.items()))
            r[m] = r.get(m, 0) + c1 * c2
            if r[m] == 0:
                del r[m]
    return r


def _atom(name: str) -> Poly:
    return {((name, 1),): 1}


def poly(node: ast.AST) -> Poly:
    if isinstance(node, ast.Constant) and isinstance(node.value, int) and not isinstance(node.value, bool):
        return {(): node.value} if node.value else {}
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.USub):
        return {m: -c for m, c in poly(node.operand).items()}
    if isinstance(node, ast.BinOp):
        if isinstance(node.op, ast.Add):
            return _padd(poly(node.left), poly(node.right))
        if isinstance(node.op, ast.Sub):
            return _padd(poly(node.left), poly(node.right), -1)
        if isinstance(node.op, ast.Mult):
            return _pmul(poly(node.left), poly(node.right))
        if isinstance(node.op, ast.Pow) and isinstance(node.right, ast.Constant) and isinstance(node.right.value, int) and node.right.value >= 0:
            r: Poly = {(): 1}
            for _ in range(node.right.value):
                r = _pmul(r, poly(node.left))
            return r
        if isinstance(node.op, (ast.FloorDiv, ast.Mod, ast.Div)):
            op = {ast.FloorDiv: "//", ast.Mod: "%", ast.Div: "/"}[type(node.op)]
            left, right = poly(node.left), poly(node.right)
            if op == "//":
                return floordiv(left, right)
            # constant folding
            if all(m == () for m in left) and all(m == () for m in right) and right.get((), 0):
                lv, rv = left.get((), 0), right[()]
                if op == "//":
                    return {(): lv // rv} if lv // rv else {}
                if op == "%":
                    return {(): lv % rv} if lv % rv else {}
            return _atom(f"({pstr(left)}){op}({pstr(right)})")
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id in ("max", "min"):
        args = sorted(pstr(poly(a)) for a in node.args)
        return _atom(f"{node.func.id}({', '.join(args)})")
    return _atom(ast.unparse(node).replace(" ", ""))


FLOORS: Dict[str, Tuple[Poly, Poly]] = {}  # atom text -> (numerator, divisor)


def _mono_div(m: Tuple[Tuple[str, int], ...], d: Tuple[Tuple[str, int], ...]):
    dm = dict(m)
    for a, e in d:
        if dm.get(a, 0) < e:
            return None
        dm[a] -= e
        if dm[a] == 0:
            del dm[a]
    return tuple(sorted(dm.items()))


def floordiv(num: Poly, den: Poly) -> Poly:
    """Normal form of num // den (dims are non-negative ints): terms that are whole multiples
    of a monomial divisor leave the floor ((H - 2)//2 + 1 == H//2), nested floors with
    literal divisors merge (H//2//2 == H//4), constants fold."""
    if len(den) == 1:
        (dm, dc), = den.items()
        if dc > 0:
            out: Poly = {}
            rest: Poly = {}
            for m, c in num.items():
                q = _mono_div(m, dm)
                if q is not None and (c % dc == 0 or (m == () and dm == ())):
                    if m == () and dm == ():
                        qq, rr = c // dc, c % dc
                        if qq:
                            out = _padd(out, {(): qq})
                        if rr:
                            rest = _padd(rest, {(): rr})
                        continue
                    out = _padd(out, {q: c // dc})
                else:
                    rest = _padd(rest, {m: c})
            if not rest:
                return out
            # (Y // k1) // k2 == Y // (k1*k2)
            if len(rest) == 1 and dm == ():
                (rm, rc), = rest.items()
                if rc == 1 and len(rm) == 1 and rm[0][1] == 1 and rm[0][0] in FLOORS:
                    inum, iden = FLOORS[rm[0][0]]
                    if len(iden) == 1 and list(iden)[0] == ():
                        return _padd(out, floordiv(inum, {(): iden[()] * dc}))
            if dm == () and all(m == () for m in rest):
                return _padd(out, {(): sum(rest.values()) // dc} if sum(rest.values()) // dc else {})
            key = f"({pstr(rest)})//({pstr(den)})"
            FLOORS[key] = (rest, den)
            return _padd(out, _atom(key))
    key = f"({pstr(num)})//({pstr(den)})"
    FLOORS[key] = (num, den)
    return _atom(key)


def pstr(p: Poly) -> str:
    if not p:
        return "0"
    terms = []
    for m, c in sorted(p.items()):
        body = "*".join(a if e == 1 else f"{a}**{e}" for a, e in m)
        terms.append(f"{c}" if not body else (body if c == 1 else f"{c}*{body}"))
    return " + ".join(terms)


def norm_dim(expr: str, where: Dict[str, str]) -> str:
    expr = expr.strip()
    if expr == "_":
        return "_"
    return pstr(poly(_parse(expr, where)))


def norm_rest(text: str, where: Dict[str, str]) -> str:
    text = text.strip()
    if text in ("", "*"):
        return "*"
    if text == "_":
        return "_"
    if text in where:
        text = where[text]
    text = text.replace(" ", "")
    m = re.match(r"^broadcast\((.*)\)$", text)
    if m:  # argument order does not matter
        text = "broadcast(" + ",".join(sorted(_split_top(m.group(1)))) + ")"
    return text


def parse_where(s: str) -> Tuple[str, Dict[str, str]]:
    m = re.search(r"\bwhere\b(.*)$", s)
    if not m:
        return s.strip(), {}
    env: Dict[str, str] = {}
    for part in _split_top(m.group(1)):
        k, _, v = part.partition("=")
        env[k.strip()] = v.strip()
    return s[: m.start()].strip(), env


def parse_shape_spec(spec: str) -> Dict[str, Any]:
    """`[A, *R, 10] where …` → {"kind": "shape", "prefix": [...], "rest": str|None, "suffix": [...]}."""
    body, where = parse_where(spec)
    body, assume = split_if(body)
    assume_n = sorted(norm_constraint(a, where) for a in assume)
    if body == "?":
        return {"kind": "unknown"}
    if body == "none":
        return {"kind": "none"}
    if not (body.startswith("[") and body.endswith("]")):
        raise ValueError(f"bad shape {spec!r}")
    items = _split_top(body[1:-1])
    prefix, suffix, rest = [], [], None
    for it in items:
        if it.startswith("*"):
            if rest is not None:
                raise ValueError(f"two rests in {spec!r}")
            rest = norm_rest(it[1:], where)
        elif rest is None:
            prefix.append(norm_dim(it, where))
        else:
            suffix.append(norm_dim(it, where))
    return {"kind": "shape", "prefix": prefix, "rest": rest, "suffix": suffix, "text": body, "assume": assume_n}


def split_if(body: str) -> Tuple[str, List[str]]:
    """`[…] if x.dim() >= 2, y.dim() >= 2`: the shape holds under these assumptions."""
    depth = 0
    for i in range(len(body)):
        ch = body[i]
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        elif depth == 0 and body.startswith(" if ", i):
            return body[:i].strip(), _split_top(body[i + 4:])
    return body.strip(), []


def engine_shape(sh: Optional[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
    """Engine form: {"dims": [str], "rest": null | {"at": i, "text": str}}."""
    if sh is None:
        return None
    dims = [norm_dim(d, {}) for d in sh["dims"]]
    if sh.get("rest") is None:
        return {"prefix": dims, "rest": None, "suffix": []}
    at = sh["rest"]["at"]
    return {"prefix": dims[:at], "rest": norm_rest(sh["rest"]["text"], {}), "suffix": dims[at:]}


def shape_text(sh: Optional[Dict[str, Any]]) -> str:
    if sh is None:
        return "not a tensor"
    parts = list(sh["dims"])
    if sh.get("rest") is not None:
        parts.insert(sh["rest"]["at"], "*" + ("" if sh["rest"]["text"] == "*" else sh["rest"]["text"]))
    return "[" + ", ".join(parts) + "]"


def shape_problem(spec: Dict[str, Any], got: Optional[Dict[str, Any]]) -> str:
    e = engine_shape(got)
    if spec["kind"] == "unknown":
        if e is None or (e["rest"] == "*" and not e["prefix"] and not e["suffix"]):
            return ""
        return f"shape {shape_text(got)}, want unknown"
    if spec["kind"] == "none":
        return "" if e is None else f"shape {shape_text(got)}, want not a tensor"
    if e is None:
        return f"not a tensor, want {spec['text']}"
    if (spec["rest"] is None) != (e["rest"] is None) or len(spec["prefix"]) != len(e["prefix"]) or len(spec["suffix"]) != len(e["suffix"]):
        return f"shape {shape_text(got)}, want {spec['text']}"
    for w, g in list(zip(spec["prefix"], e["prefix"])) + list(zip(spec["suffix"], e["suffix"])):
        if w != "_" and w != g:
            return f"shape {shape_text(got)}, want {spec['text']} ({g} is not {w})"
    if spec["rest"] not in (None, "_") and spec["rest"] != e["rest"]:
        return f"shape {shape_text(got)}, want {spec['text']} (rest {e['rest']} is not {spec['rest']})"
    got_assume = sorted(norm_constraint(a) for a in got.get("assume", []))
    if got_assume != spec["assume"]:
        return f"shape {shape_text(got)} assumes {got.get('assume', [])}, want {spec['assume'] or 'no assumptions'}"
    return ""


def directive_lines(src: str) -> List[int]:
    """Effective line of each source line (1-based list index i -> line): a line holding only a
    `#@` comment applies to the nearest code line above it."""
    eff: List[int] = [0]
    last_code = 0
    for i, line in enumerate(src.splitlines(), 1):
        st = line.strip()
        if st.startswith("#@") and last_code:
            eff.append(last_code)
        else:
            if st and not st.startswith("#"):
                last_code = i
            eff.append(i)
    return eff


def norm_constraint(text: str, where: Optional[Dict[str, str]] = None) -> str:
    text, conds = split_if(text.strip())
    where = where or {}
    if conds:
        return norm_constraint(text, where) + " if " + ", ".join(sorted(norm_constraint(c, where) for c in conds))
    m = re.match(r"^(.*?)\s*==\s*(.*)$", text)
    if m and " in " not in text:
        try:
            p = _padd(poly(_parse(m.group(1), where)), poly(_parse(m.group(2), where)), -1)
        except SyntaxError:
            return text.replace(" ", "")
        # orientation: the first term positive
        if p and sorted(p.items())[0][1] < 0:
            p = {k: -v for k, v in p.items()}
        return "eq:" + pstr(p)
    return text.replace(" ", "")


class ShapeExpectations:
    def __init__(self) -> None:
        self.shapes: List[Tuple[int, Optional[int], str, Dict[str, Any]]] = []  # line, at, name, spec
        self.dims: List[Tuple[int, Optional[int], str, str]] = []
        self.requires: Dict[int, List[str]] = {}
        self.errors: Dict[int, str] = {}
        self.eff: Optional[List[int]] = None

    def any(self) -> bool:
        return bool(self.shapes or self.dims or self.requires or self.errors)

    def parse_line(self, i: int, line: str) -> bool:
        if self.eff is not None and i < len(self.eff):
            i = self.eff[i]
        m = re.search(r"#@(shape|dim)(?:@L(\d+))?\s+(.*)$", line)
        if m and not line[m.start():].startswith("#@shape-error"):
            at = int(m.group(2)) if m.group(2) else None
            for clause in _split_top(m.group(3), ";"):
                cm = re.match(r"\s*([\w.]+)\s*:\s*(.*)$", clause)
                if not cm:
                    raise ValueError(f"line {i}: bad #@{m.group(1)} clause {clause!r}")
                if m.group(1) == "shape":
                    self.shapes.append((i, at, cm.group(1), parse_shape_spec(cm.group(2))))
                else:
                    body, where = parse_where(cm.group(2))
                    self.dims.append((i, at, cm.group(1), "?" if body.strip() == "?" else norm_dim(body, where)))
            return True
        m = re.search(r"#@requires\s+(.*)$", line)
        if m:
            body, where = parse_where(m.group(1))
            new = [] if body.strip() == "none" else [norm_constraint(c, where) for c in _split_top(body, ";")]
            self.requires[i] = self.requires.get(i, []) + new
            return True
        m = re.search(r"#@shape-error\b\s*(.*)$", line)
        if m:
            self.errors[i] = m.group(1).strip()
            return True
        return False

    def check(self, res: Dict[str, Any]) -> List[str]:
        fails: List[str] = []
        rows = res.get("rows", [])

        def runs(line: int, name: str, at: Optional[int]):
            here = [r for r in rows if r["line"] == line and r["name"] == name]
            if at is not None:
                return [r for r in here if at in r.get("chain", [])]
            entry = [r for r in here if r.get("entry")]
            return entry or here

        for line, at, name, spec in self.shapes:
            rs = runs(line, name, at)
            if not rs:
                fails.append(f"L{line} shape {name}: statement never executed" + (f" from L{at}" if at else ""))
                continue
            for r in rs:
                p = shape_problem(spec, r.get("shape"))
                if p:
                    fails.append(f"L{line} shape {name}: {p}")
                    break
        for line, at, name, want in self.dims:
            rs = runs(line, name, at)
            if not rs:
                fails.append(f"L{line} dim {name}: statement never executed")
                continue
            for r in rs:
                got = r.get("dim")
                g = "?" if got is None else norm_dim(got, {})
                if want != "_" and g != want:
                    fails.append(f"L{line} dim {name}: {got if got is not None else 'unknown'}, want {want}")
                    break
        reqs = res.get("requires", [])
        for line, want in self.requires.items():
            got = sorted({norm_constraint(r["text"]) for r in reqs if r["line"] == line or line in r.get("chain", [])})
            if sorted(set(want)) != got:
                texts = sorted({r["text"] for r in reqs if r["line"] == line or line in r.get("chain", [])})
                fails.append(f"L{line} requires: got {texts or 'none'}, want {want or 'none'}")
        errs = res.get("shape_errors", [])
        for e in errs:
            lines = [e["line"], *e.get("chain", [])]
            hit = [ln for ln in lines if ln in self.errors]
            if not hit:
                fails.append(f"L{e['line']}: unexpected shape error: {e['message']}")
            elif self.errors[hit[0]] and self.errors[hit[0]] not in e["message"]:
                fails.append(f"L{hit[0]}: shape error {e['message']!r} does not mention {self.errors[hit[0]]!r}")
        for line in self.errors:
            if not any(line == e["line"] or line in e.get("chain", []) for e in errs):
                fails.append(f"L{line}: expected a shape error, got none")
        return fails
