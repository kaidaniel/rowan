"""A method's summary records the attribute reads on its receiver's parts (`self._partial.write`)
and replays them on the objects a call passes. A stdlib object the models simulate (a StringIO, a
compiled pattern) has its methods from the model, not from its attribute row (which holds the
model's own state): replayed on one, the reads were classified against that row and reported
`write` / `seek` / `read` / `match` definitely missing (email's `BufferedSubFile.push`, once the
call budget was spent). With the budget at 0 every call here uses its summary."""
# (summaries are gone since: every call runs at its call site)
import re
from io import StringIO


class Buf:
    def __init__(self):
        self._partial = StringIO()
        self._pat = re.compile("a+")
        self._lines = ["x"]

    def push(self, data):
        self._partial.write(data)  #@ sound write
        self._partial.seek(0)  #@ sound seek
        text = self._partial.read()  #@ sound read
        self._lines.append(text)
        return self._pat.match(data)  #@ sound match


b = Buf()
m = b.push("aa")
print(m, b._lines)
