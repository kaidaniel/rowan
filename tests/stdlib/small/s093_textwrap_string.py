# string.Template reading mapping keys (keywords win), safe_substitute, string constants, textwrap on literals

import string
import textwrap

LETTER = string.Template("Dear $name: you owe ${amount} $currency.")


def letter(acct):
    return LETTER.substitute(acct, currency="EUR")
#@schema param letter.acct: always={name,amount} only={name,amount}


l1 = letter({"name": "Ann", "amount": 5})                              #@value l1: exactly={"Dear Ann: you owe 5 EUR."}
l2 = letter({"name": "Bo", "amount": 1, "currency": "USD", "id": 9})   #@value l2: exactly={"Dear Bo: you owe 1 EUR."}
try:
    letter({"name": "Cy"})                                             #@ missing amount
except KeyError:
    pass
safe = string.Template("$a and $b").safe_substitute({"a": 1})         #@value safe: exactly={"1 and $b"}
try:
    string.Template("$x").substitute({})                               #@ missing x
except KeyError:
    pass
dg = string.digits                                                     #@value dg: exactly={"0123456789"}
nl = len(string.ascii_lowercase)                                       #@value nl: exactly={26}
lines = textwrap.wrap("the quick brown fox", width=10)
l0 = lines[0]                                                          #@value l0: exactly={"the quick"}
nlines = len(lines)                                                    #@value nlines: exactly={2}
sh = textwrap.shorten("Hello  big   world", width=12)                  #@value sh: exactly={"Hello [...]"}
dd = textwrap.dedent("  a\n  b")                                       #@value dd: exactly={"a\nb"}
ind = textwrap.indent("x\ny", "> ")                                    #@value ind: exactly={"> x\n> y"}
