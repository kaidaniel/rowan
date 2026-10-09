"""Hypothesis: iterating an Enum class gives its members in order; `.name` and `.value` are
known.

Expectations were written from the program text before running the engine."""

from enum import Enum


class Color(Enum):
    RED = "red"
    GREEN = "green"


by_name = {c.name: c.value for c in Color}   #@row by_name: present={RED,GREEN} definite={RED,GREEN} closed
by_value = {c.value: c for c in Color}       #@row by_value: present={red,green} definite={red,green} closed
k = Color.RED.value
d = {k: 1}                                   #@row d: present={red} definite={red} closed
