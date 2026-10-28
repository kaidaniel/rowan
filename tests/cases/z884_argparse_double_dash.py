"""argparse: `--` ends the options — every token after it is a positional, even one that
looks like an option. (Before, `--` was looked up as an option, found none, and the parse
was taken to exit with an error.)

Expectations were written from the program text before running the engine."""
import argparse

ap = argparse.ArgumentParser(prog="p")
ap.add_argument("-v", action="store_true")
ap.add_argument("files", nargs="*")
ns = ap.parse_args(["--", "-v", "x"])
files = ns.files
n = len(files)
#@value n: exactly={2}
first = files[0]
#@value first: exactly={"-v"}
flag = ns.v
#@value flag: exactly={False}
