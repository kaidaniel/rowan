"""A function defined inside a generator is a new closure object at every run of the
generator. The tabulation budget belongs to the function's code, not to each closure object:
with a budget of 4, `is_sep` runs per call site 4 times in all (here: the 3 lines of the first
loop and the first line of the second), and every later call uses its summary (True or
False). Keyed by closure object, each of the 8 loops' closures had a budget of its own (up to
24 runs of `is_sep`). The work: 8 generator runs (a generator with calls runs per call site)
plus 4 runs each of `is_sep` and `shape`, with a margin."""
#@limits tab=4
#@tabulations <= 20


def parse(lines, sep):
    def is_sep(line):
        if line == sep:
            return True
        if line.startswith(sep + "-"):
            return True
        return False

    for line in lines:
        if is_sep(line):
            yield {"sep": line}
        else:
            yield {"text": line}


def shape(rec):
    return "S" if "sep" in rec else "T"


s1 = ""
for rec in parse(["a", "--", "b"], "--"):
    s1 += shape(rec)

s2 = ""  #@value s1: exactly={"TST"}
for rec in parse(["==", "q"], "=="):
    s2 += shape(rec)

s3 = ""  #@value s2: includes={"ST"}
for rec in parse(["x", "---y", "z"], "--"):
    s3 += shape(rec)

s4 = ""  #@value s3: includes={"TST"}
for rec in parse(["##", "##-", "k"], "##"):
    s4 += shape(rec)

s5 = ""  #@value s4: includes={"SST"}
for rec in parse(["m", "n", "o"], "--"):
    s5 += shape(rec)

s6 = ""  #@value s5: includes={"TTT"}
for rec in parse(["--", "--", "--"], "--"):
    s6 += shape(rec)

s7 = ""  #@value s6: includes={"SSS"}
for rec in parse(["a", "b", "=="], "=="):
    s7 += shape(rec)

s8 = ""  #@value s7: includes={"TTS"}
for rec in parse(["++", "+", "++-"], "++"):
    s8 += shape(rec)

print(s1, s2, s3, s4, s5, s6, s7, s8)  #@value s8: includes={"STS"}
