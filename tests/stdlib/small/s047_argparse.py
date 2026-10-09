# argparse with dest=, defaults, store_true, append, type=int; parse_args with explicit argv lists
import argparse


def build():
    p = argparse.ArgumentParser(prog="tool")
    p.add_argument("src")
    p.add_argument("--out", dest="output", default="-")
    p.add_argument("-v", "--verbose", action="store_true")
    p.add_argument("--level", type=int, default=1)
    p.add_argument("--tag", action="append")
    return p


def run(argv):
    args = build().parse_args(argv)
    _ = 0                                        #@row args: present={src,output,verbose,level,tag} definite={src,output,verbose,level,tag} closed
    args.output                                  #@ ok output
    v = args.verbose                             #@value v: exactly={True,False}
    lvl = args.level                             #@value lvl: exactly={1,3}
    d = vars(args)                               #@row d: present={src,output,verbose,level,tag} definite={src,output,verbose,level,tag} closed
    return args


if __name__ == "__main__":
    a = run(["a.txt"])
    o = a.output                                 #@value o: exactly={"-"}
    tg = a.tag                                   #@value tg: exactly={None}
    b = run(["b", "--out", "o", "-v", "--level", "3", "--tag", "x"])
    try:
        b.out                                    #@ missing out
    except AttributeError:
        pass
    s = b.src                                    #@value s: exactly={b}
