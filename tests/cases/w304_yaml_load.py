"""Hypothesis: `yaml.safe_load` is an input like json.

Expectations were written from the program text before running the engine."""
import yaml

cfg = yaml.safe_load(open("c.yml"))
cfg["name"]                                  #@ input name
#@schema yaml.safe_load@L6: always={name}
