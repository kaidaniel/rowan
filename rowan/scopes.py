"""Lexical scope analysis: which names of a function are locals, cells (captured
by nested functions), free (captured from an enclosing function), or globals.

Comprehensions are *not* separate scopes here: their targets are treated as
locals of the enclosing function (the interpreter scopes them with an env
overlay), except that a comprehension nested directly in a class body or at
module level simply uses the enclosing frame.
"""
from __future__ import annotations

import ast
from dataclasses import dataclass, field
from typing import Dict, Optional, Set

FunctionNode = (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)


@dataclass
class Scope:
    node: ast.AST
    kind: str                           # module | function | class
    parent: Optional["Scope"]
    locals: Set[str] = field(default_factory=set)
    globals_decl: Set[str] = field(default_factory=set)
    nonlocals_decl: Set[str] = field(default_factory=set)
    uses: Set[str] = field(default_factory=set)
    children: list = field(default_factory=list)
    free: Set[str] = field(default_factory=set)      # computed
    cells: Set[str] = field(default_factory=set)     # computed

    def is_local(self, name: str) -> bool:
        return name in self.locals and name not in self.globals_decl and name not in self.nonlocals_decl


class _Collector(ast.NodeVisitor):
    def __init__(self, scopes: Dict[ast.AST, Scope], scope: Scope):
        self.scopes = scopes
        self.scope = scope

    # -- scope boundaries ---------------------------------------------------
    def _enter(self, node: ast.AST, kind: str) -> Scope:
        s = Scope(node, kind, self.scope)
        self.scopes[node] = s
        self.scope.children.append(s)
        return s

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self.scope.locals.add(node.name)
        for d in node.decorator_list:
            self.visit(d)
        for dflt in node.args.defaults + [d for d in node.args.kw_defaults if d is not None]:
            self.visit(dflt)
        if node.returns is not None:
            self.visit(node.returns)
        inner = self._enter(node, "function")
        sub = _Collector(self.scopes, inner)
        for a in _all_args(node.args):
            inner.locals.add(a.arg)
            if a.annotation is not None:
                self.visit(a.annotation)
        for stmt in node.body:
            sub.visit(stmt)

    visit_AsyncFunctionDef = visit_FunctionDef  # type: ignore[assignment]

    def visit_Lambda(self, node: ast.Lambda) -> None:
        for dflt in node.args.defaults + [d for d in node.args.kw_defaults if d is not None]:
            self.visit(dflt)
        inner = self._enter(node, "function")
        sub = _Collector(self.scopes, inner)
        for a in _all_args(node.args):
            inner.locals.add(a.arg)
        sub.visit(node.body)

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        self.scope.locals.add(node.name)
        for d in node.decorator_list:
            self.visit(d)
        for b in node.bases:
            self.visit(b)
        for kw in node.keywords:
            self.visit(kw.value)
        inner = self._enter(node, "class")
        sub = _Collector(self.scopes, inner)
        for stmt in node.body:
            sub.visit(stmt)

    # -- names ------------------------------------------------------------
    def visit_Name(self, node: ast.Name) -> None:
        if isinstance(node.ctx, (ast.Store, ast.Del)):
            self.scope.locals.add(node.id)
        else:
            self.scope.uses.add(node.id)

    def visit_Global(self, node: ast.Global) -> None:
        self.scope.globals_decl.update(node.names)

    def visit_Nonlocal(self, node: ast.Nonlocal) -> None:
        self.scope.nonlocals_decl.update(node.names)
        self.scope.uses.update(node.names)

    def visit_Import(self, node: ast.Import) -> None:
        for a in node.names:
            self.scope.locals.add((a.asname or a.name).split(".")[0])

    def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
        for a in node.names:
            if a.name != "*":
                self.scope.locals.add(a.asname or a.name)

    def visit_ExceptHandler(self, node: ast.ExceptHandler) -> None:
        if node.name:
            self.scope.locals.add(node.name)
        self.generic_visit(node)

    def visit_NamedExpr(self, node: ast.NamedExpr) -> None:
        self.scope.locals.add(node.target.id)  # type: ignore[attr-defined]
        self.visit(node.value)

    def visit_MatchAs(self, node: ast.MatchAs) -> None:
        if node.name:
            self.scope.locals.add(node.name)
        self.generic_visit(node)


def _all_args(args: ast.arguments):
    out = list(args.posonlyargs) + list(args.args) + list(args.kwonlyargs)
    if args.vararg:
        out.append(args.vararg)
    if args.kwarg:
        out.append(args.kwarg)
    return out


def _resolve(scope: Scope) -> None:
    """Compute free/cells bottom-up."""
    for ch in scope.children:
        _resolve(ch)
    if scope.kind == "function":
        for name in scope.uses | scope.nonlocals_decl:
            if scope.is_local(name) or name in scope.globals_decl:
                continue
            # look for a defining function scope outward (skipping class scopes)
            p = scope.parent
            path = []
            while p is not None:
                if p.kind == "function":
                    if p.is_local(name):
                        p.cells.add(name)
                        scope.free.add(name)
                        for mid in path:
                            mid.free.add(name)
                        break
                    path.append(p)
                elif p.kind == "module":
                    break
                p = p.parent
    # names free in children that pass through this function
    for ch in scope.children:
        for name in ch.free:
            if scope.kind == "function" and not scope.is_local(name) and name not in scope.free:
                # passes through to an outer function
                p = scope.parent
                while p is not None and p.kind != "module":
                    if p.kind == "function" and p.is_local(name):
                        scope.free.add(name)
                        break
                    p = p.parent


def analyse(tree: ast.Module) -> Dict[ast.AST, Scope]:
    scopes: Dict[ast.AST, Scope] = {}
    root = Scope(tree, "module", None)
    scopes[tree] = root
    coll = _Collector(scopes, root)
    for stmt in tree.body:
        coll.visit(stmt)
    _resolve(root)
    return scopes
