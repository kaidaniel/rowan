# rowan for VS Code

> **⚠ Experimental.** rowan is a research prototype. Expect rough edges, and expect large
> files (thousands of lines, big packages) to be slow or to time out.

rowan is an **abstract interpreter for Python**: it runs your program on *sets of possible
values* instead of real ones. **Hover over any variable** to see what it can hold — which
literals, which dict keys, which are certain and which only maybe there. Accesses that can
fail show up as diagnostics.

```python
def make_user(name, admin):
    user = {"name": name, "active": True}
    if admin:
        user["role"] = "admin"
    return user

ana = make_user("ana", True)    # hover: {"active": True, "name": "ana", "role": "admin"}
bo = make_user("bo", False)     # hover: {"active": True, "name": "bo"}

bo["role"]                      # ✗ 'role' definitely missing

for u in [ana, bo]:
    print(u["role"])            # ⚠ 'role' possibly missing: ✓ for ana, ✗ for bo
```

No setup: the analyser is bundled with the extension. Files are analysed when opened or
saved, or with **Rowan: Analyse current file**.
