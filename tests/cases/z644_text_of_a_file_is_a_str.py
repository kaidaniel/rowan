"""What `pathlib` reads of a file is a str (`read_text`; `read_bytes`: bytes), and the characters
and items of a str not known are str: `sorted(set(text))` is a list of str, `text[0]` a str.
They were `?` (the hover named them by the call, `Path("/tmp/rowan_z644.txt").read_text()`, without a type).

Expectations were written from the program text before running the engine."""
from pathlib import Path

Path("/tmp/rowan_z644.txt").write_text("ab c")
text = Path("/tmp/rowan_z644.txt").read_text()      #@value text: types={str}
data = Path("/tmp/rowan_z644.txt").read_bytes()     #@value data: types={bytes}
chars = sorted(set(text))
first = chars[0]                            #@value first: types={str}
c0 = text[0]                                #@value c0: types={str}
for ch in text:
    last = ch                               #@value last: types={str}
b0 = data[0]                                #@value b0: types={int}
