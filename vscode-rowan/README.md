> **⚠ Experimental.** `rowan` is in development. Expect it to crash on large files and outputs to be at times poorly explained.

rowan is an **abstract interpreter for Python**: it runs your program on *sets of possible
values* instead of real ones. **Hover over any variable** to see what it can hold - which
literals, which dict keys, which are certain and which only maybe there.

```python
def make_user(name, admin):
    user = {"name": name, "active": True}
    if admin:
        user["role"] = "admin"
    return user

ana = make_user("ana", True)    # hover: {"active": True, "name": "ana", "role": "admin"}
bo = make_user("bo", False)     # hover: {"active": True, "name": "bo"}
```

Besides python builtins, rowan also has models for pytorch, pandas, pydantic, and a few other libraries. Local imports are resolved but other external dependencies are not analysed.

Files are analysed when opened or
saved, or with **Rowan: Analyse current file**.
