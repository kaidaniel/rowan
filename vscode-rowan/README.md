> **⚠ Experimental.** `rowan` is in development. Expect it to crash on large files and outputs to be at times poorly explained.

rowan is an **abstract interpreter for Python**: it runs your program on *sets of possible
values* instead of real ones. **Hover over any variable** to see what it can hold - which
literals, which dict keys, which are certain and which only maybe there.

```python
def add_admin(user, admin):
    if admin:
        user["role"] = "admin"
    return user                               # hover: {"name": "Anna"} | {"name": "Bert", "role": "admin"}

anna = add_admin({"name": "Anna"}, True)      # hover: {"name": "Anna", "role": "admin"}
bert = make_user({"name": "Bert"}, False)     # hover: {"name": "Bert"}
```

Besides python builtins, rowan also has models for pytorch, pandas, pydantic, and a few other libraries. Local imports are resolved but other external dependencies are not analysed.

Files are analysed when opened or
saved, or with **Rowan: Analyse current file**. When a whole file takes longer than 10 seconds,
rowan analyses each function when you first hover in it, for any call of it (usually well under
a second), and says so in the status bar.

A hover shows `name = value` (a parameter: `name = (parameter)`, with its type or default when
known: `(parameter) int(?)`), then what the code needs of a parameter (**Requirements**:
`row["id"]`, `row["since"] (only if admin)`, `x.dim() == 3`), what the program's own calls give or
pass (**In Calls**: `{"id": 2} on line 24: ✗ row["since"]`), where its objects were made
(**Allocation Sites**: `created on line 12`, `never has key "b" here`), and what each mark it uses
means (**Notes**; last, when the analysis was cut or ran on demand). Accesses that may fail are
diagnostics.

Source, documentation and issues: [github.com/kaidaniel/rowan](https://github.com/kaidaniel/rowan).
