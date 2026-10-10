# Deploy CLI: argparse over explicit argv lists, a JSON config (literal default text or text from os.environ), dataclass settings merged from both, a per-job report dict
import argparse
import json
import os
from dataclasses import asdict, dataclass, field

DEFAULT_CONFIG = '{"name": "nightly", "retries": 2, "targets": [{"host": "a.example", "port": 2222}, {"host": "b.example"}], "notify": {"email": "ops@example.org"}}'


@dataclass
class Target:
    host: str
    port: int = 22
    enabled: bool = True


@dataclass
class Settings:
    name: str
    retries: int
    verbose: bool = False
    dry_run: bool = False
    targets: list = field(default_factory=list)
    notify: dict = field(default_factory=dict)


def build_parser():
    p = argparse.ArgumentParser(prog="deploy")
    p.add_argument("command", choices=["run", "check"])
    p.add_argument("-v", "--verbose", action="store_true")
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--retries", type=int)
    p.add_argument("--only", action="append", default=[])
    return p


def parse_args(argv):
    args = build_parser().parse_args(argv)
    #@row args: present={command,verbose,dry_run,retries,only} definite={command,verbose,dry_run,retries,only} closed
    return args


def colour_of(args):
    return args.colour                           #@ missing colour


def parse_target(raw):
    t = Target(host=raw["host"], port=int(raw.get("port", 22)), enabled=raw.get("enabled", True))
    return t
#@schema param parse_target.raw: always={host} only={host}


def settings_from(cfg, args):
    s = Settings(name=cfg["name"], retries=cfg.get("retries", 0))
    if args.retries is not None:
        s.retries = args.retries
    s.verbose = args.verbose
    s.dry_run = args.dry_run
    s.targets = [parse_target(t) for t in cfg.get("targets", [])]
    if args.only:
        s.targets = [t for t in s.targets if t.host in args.only]
    if "notify" in cfg:
        s.notify = dict(cfg["notify"])           #@ ok notify
    _ = 0  #@row s: present={name,retries,verbose,dry_run,targets,notify} definite={name,retries,verbose,dry_run,targets,notify} closed
    return s


def run_job(settings):
    results = []
    for t in settings.targets:
        if not t.enabled:
            status = "skipped"
        elif settings.dry_run:
            status = "dry-run"
        else:
            status = "ok"
        entry = {"host": t.host, "port": t.port, "status": status}
        #@row entry: present={host,port,status} definite={host,port,status} closed
        results.append(entry)
    report = {"job": settings.name, "attempts": settings.retries + 1, "results": results}
    if settings.verbose:
        report["settings"] = asdict(settings)
    if settings.notify.get("email"):
        report["notify"] = settings.notify["email"]
    _ = 0  #@row report: present={job,attempts,results,settings,notify} definite={job,attempts,results} closed
    return report


def main(argv, config_text=None):
    args = parse_args(argv)
    text = config_text if config_text is not None else DEFAULT_CONFIG
    cfg = json.loads(text)
    settings = settings_from(cfg, args)
    if args.command == "check":
        return {"job": settings.name, "targets": len(settings.targets)}
    return run_job(settings)
#@schema json.loads@L92: always={name} only={name}
#@schema json.loads@L92['targets']['*']: always={host} only={host}


if __name__ == "__main__":
    r1 = main(["run", "-v"])
    #@row r1: present={job,attempts,results,settings,notify} definite={job,attempts,results,settings,notify} closed
    att = r1["attempts"]                         #@value att: exactly={3}
    hosts = tuple(x["host"] for x in r1["results"])   #@value hosts: exactly={("a.example","b.example")}
    port = r1["results"][0]["port"]              #@value port: exactly={2222}
    r2 = main(["run", "--dry-run", "--retries", "5", "--only", "b.example"])
    #@row r2: present={job,attempts,results,notify} definite={job,attempts,results,notify} absent={settings} closed
    st = r2["results"][0]["status"]              #@value st: exactly={dry-run}
    att2 = r2["attempts"]                        #@value att2: exactly={6}
    r3 = main(["check"])                         #@row r3: present={job,targets} definite={job,targets} closed
    n = r3["targets"]                            #@value n: exactly={2}
    r4 = main(["run"], '{"name": "adhoc", "targets": [{"host": "c.example", "enabled": false}]}')
    #@row r4: present={job,attempts,results} definite={job,attempts,results} absent={settings,notify} closed
    st4 = r4["results"][0]["status"]             #@value st4: exactly={skipped}
    r5 = main(["run", "-v"], os.environ.get("DEPLOY_CONFIG"))
    #@row r5: present={job,attempts,results,settings,notify} definite={job,attempts,results,settings} closed
    try:
        parse_args(["deploy"])
    except SystemExit:
        pass
    try:
        colour_of(parse_args(["check", "-v"]))
    except AttributeError:
        pass
