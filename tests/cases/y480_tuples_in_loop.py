"""Tuples built in a loop and unpacked later keep their positions: a tuple object folded into
the loop's summary object kept no positions, so `string, charset = t` gave each name the
join of both elements, and `string.splitlines()` was reported definitely missing for the
charset objects (email's `Header._normalize` / `Header.encode`)."""
import sys

SPACE = " "


class Charset:
    def __init__(self, name):
        self.name = name


class Header:
    def __init__(self, s=None, charset=None):
        self._chunks = []
        if s is not None:
            self.append(s, charset)

    def append(self, s, charset=None):
        if charset is None:
            charset = Charset("us-ascii")
        self._chunks.append((s, charset))

    def _normalize(self):
        chunks = []
        last_charset = None
        last_chunk = []
        for string, charset in self._chunks:
            if charset == last_charset:
                last_chunk.append(string)
            else:
                if last_charset is not None:
                    chunks.append((SPACE.join(last_chunk), last_charset))
                last_chunk = [string]
                last_charset = charset
        if last_chunk:
            chunks.append((SPACE.join(last_chunk), last_charset))
        self._chunks = chunks

    def encode(self):
        self._normalize()
        out = []
        for string, charset in self._chunks:
            lines = string.splitlines()          #@ sound splitlines
            out.append(lines)
        return out


h = Header(sys.argv[0])
h.append("x")
r = h.encode()
