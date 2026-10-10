"""argparse choices computed from a module global inside the function that builds the parser
(`sorted(KEYS)`): checked against each call's argv, so an invalid command exits."""
import argparse

KEYS = {"regions": "region", "categories": "category", "reps": "rep"}


def build_parser():
    p = argparse.ArgumentParser(prog="report")
    p.add_argument("command", choices=sorted(KEYS))
    p.add_argument("--top", type=int, default=2)
    return p


def run(argv):
    args = build_parser().parse_args(argv)
    return KEYS[args.command]


if __name__ == "__main__":
    a = run(["regions"])                         #@value a: exactly={"region"}
    b = run(["reps", "--top", "1"])              #@value b: exactly={"rep"}
    try:
        run(["totals"])
        bad = False
    except SystemExit:
        bad = True
    _ = 0                                        #@value bad: exactly={True}
