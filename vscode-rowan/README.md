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

A hover shows the value as Python, where the object was created, what the code reads of a
value that comes from outside (an argument, `json.loads(…)`), the accesses on the line, and a
one-line legend of the marks it uses. For `u = make_user(...)` followed by `u["role"]`:

````
rowan · u

```python
{"id": 7, "name": "Ada Lovelace", "role"?: "admin"}
```

- created on line 4
- ⚠ u["role"]: key "role" is only set on some paths

"k"?: set on some paths only · ⚠ fails on some paths
````

Accesses that may fail are also diagnostics: `✗` (fails on every path) as errors, `⚠` (fails
on some paths) as warnings.

Besides python builtins, rowan also has models for pytorch, pandas, pydantic, and a few other libraries. Local imports are resolved but other external dependencies are not analysed.

Files are analysed when opened or
saved, or with **Rowan: Analyse current file**.

Source, documentation and issues: [github.com/kaidaniel/rowan](https://github.com/kaidaniel/rowan).
