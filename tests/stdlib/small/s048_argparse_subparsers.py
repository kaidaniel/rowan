# argparse subparsers: each subcommand adds its own attributes; set_defaults per subcommand
import argparse

p = argparse.ArgumentParser()
p.add_argument("--debug", action="store_true")
sub = p.add_subparsers(dest="cmd", required=True)
a = sub.add_parser("add")
a.add_argument("name")
a.add_argument("--qty", type=int, default=1)
a.set_defaults(func="do_add")
r = sub.add_parser("rm")
r.add_argument("ids", nargs="+")
r.add_argument("--force", action="store_true")
r.set_defaults(func="do_rm")


def dispatch(argv):
    args = p.parse_args(argv)
    _ = 0                                        #@row args: present={debug,cmd,name,qty,func,ids,force} definite={debug,cmd,func} closed
    f = args.func                                #@value f: exactly={do_add,do_rm}
    if args.cmd == "add":
        args.qty                                 #@ ok qty
        return {"add": args.name}
    return {"rm": len(args.ids), "force": args.force}


def qty_of(args):
    return args.qty                              #@ ok qty ; missing qty


if __name__ == "__main__":
    x = dispatch(["add", "x"])                   #@row x: present={add} definite={add} closed
    y = dispatch(["--debug", "rm", "1", "2"])    #@row y: present={rm,force} definite={rm,force} closed
    n = y["rm"]                                  #@value n: exactly={2}
    for argv in (["add", "y", "--qty", "4"], ["rm", "9"]):
        try:
            qty_of(p.parse_args(argv))
        except AttributeError:
            pass
