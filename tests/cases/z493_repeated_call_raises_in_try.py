# A call that may raise, made again with the same entry in every round of a loop whose body is
# a try: each round's handler starts from the raise inside the call (a call memo would have to
# record the raise again into the round's own try, not only return the call's normal result).
import json
import os


def parse(text):
    return json.loads(text)["k"]  #@ input k


def main():
    n = len(os.environ.get("ROWAN_CASE_N", "ab"))
    text = os.environ.get("ROWAN_CASE_TEXT", '{"j": 1}')
    caught = 0
    v = None
    while n > 0:
        try:
            v = parse(text)
        except KeyError:
            caught = 1
        n -= 1
    return v, caught


v, caught = main()  #@value caught: exactly={0, 1}
#@work <= 600
