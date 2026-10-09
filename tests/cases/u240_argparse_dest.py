"""Hypothesis: argparse turns `--dry-run` into the attribute `dry_run`, honours `dest=`, and
keeps positional names.

Expectations were written from the program text before running the engine."""

import argparse

p = argparse.ArgumentParser()
p.add_argument("--dry-run", action="store_true")
p.add_argument("--out-dir", dest="target")
p.add_argument("name")
args = p.parse_args()
args.dry_run                                 #@ ok dry_run
args.target                                  #@ ok target
args.name                                    #@ ok name
args.out_dir                                 #@ missing out_dir
