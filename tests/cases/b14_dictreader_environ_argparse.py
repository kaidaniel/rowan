import argparse
import csv
import os

p = argparse.ArgumentParser()
p.add_argument("--input")
p.add_argument("-v", "--verbose", action="store_true")
p.add_argument("out_dir")
args = p.parse_args()
args.input                                      #@ ok input
args.verbose                                    #@ ok verbose
args.out_dir                                    #@ ok out_dir
args.outpt                                      #@ missing outpt

home = os.environ["HOME"]                       #@ input HOME
key = os.environ.get("API_KEY")                 #@ none API_KEY
#@schema os.environ: always={HOME}

with open(args.input) as f:
    for row in csv.DictReader(f):
        row["name"]                             #@ input name
        if row["age"]:                          #@ input age
            print(row.get("email"))             #@ none email
#@schema csv.DictReader@L20: always={name,age}
