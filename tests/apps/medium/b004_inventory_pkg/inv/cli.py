"""Command line: argparse subcommands; main builds the configuration and the connection and
dispatches through the registry with a partial."""
import argparse
from functools import partial

from inv.commands import COMMANDS
from inv.config import load_config
from inv.db import connect


def build_parser():
    p = argparse.ArgumentParser(prog="inv")
    p.add_argument("--verbose", action="store_true")
    sub = p.add_subparsers(dest="command", required=True)
    a = sub.add_parser("add")
    a.add_argument("sku")
    a.add_argument("--qty", type=int, default=1)
    a.add_argument("--reason")
    sub.add_parser("low")
    r = sub.add_parser("report")
    r.add_argument("--top", type=int)
    return p


def main(argv, conn=None, ini=None):
    args = build_parser().parse_args(argv)
    #@row args: present={verbose,command,sku,qty,reason,top} definite={verbose,command} closed
    cfg = load_config(ini) if ini else load_config()
    conn = conn or connect()
    run = partial(COMMANDS[args.command], conn, cfg)
    return run(args)
