"""A generator run whole for a loop inside a try keeps its raises for where the loop asks for
items. Raises no handler catches (only a finally sees them: ValueError, KeyError, TypeError,
OSError here) are one state for all their classes, copied per class when the try ends; each
copy then costed a footprint (the 40 records the generator changes) and a join of its own,
four times the work of one. Their footprint is taken once per state: about one footprint per
raise state and consumer (12 consumers × 40 records, ~2.4k visits; was 6.5k), with a margin."""
#@visits <= 3500
#@joins <= 3000
import json
import sys

DATA = json.loads(sys.argv[1] if len(sys.argv) > 1 else "[1, 2, 3]")
REG = {}

rec0 = {'id': 0}
rec1 = {'id': 1}
rec2 = {'id': 2}
rec3 = {'id': 3}
rec4 = {'id': 4}
rec5 = {'id': 5}
rec6 = {'id': 6}
rec7 = {'id': 7}
rec8 = {'id': 8}
rec9 = {'id': 9}
rec10 = {'id': 10}
rec11 = {'id': 11}
rec12 = {'id': 12}
rec13 = {'id': 13}
rec14 = {'id': 14}
rec15 = {'id': 15}
rec16 = {'id': 16}
rec17 = {'id': 17}
rec18 = {'id': 18}
rec19 = {'id': 19}
rec20 = {'id': 20}
rec21 = {'id': 21}
rec22 = {'id': 22}
rec23 = {'id': 23}
rec24 = {'id': 24}
rec25 = {'id': 25}
rec26 = {'id': 26}
rec27 = {'id': 27}
rec28 = {'id': 28}
rec29 = {'id': 29}
rec30 = {'id': 30}
rec31 = {'id': 31}
rec32 = {'id': 32}
rec33 = {'id': 33}
rec34 = {'id': 34}
rec35 = {'id': 35}
rec36 = {'id': 36}
rec37 = {'id': 37}
rec38 = {'id': 38}
rec39 = {'id': 39}


def items(xs):
    for x in xs:
        rec0['seen'] = x
        rec1['seen'] = x
        rec2['seen'] = x
        rec3['seen'] = x
        rec4['seen'] = x
        rec5['seen'] = x
        rec6['seen'] = x
        rec7['seen'] = x
        rec8['seen'] = x
        rec9['seen'] = x
        rec10['seen'] = x
        rec11['seen'] = x
        rec12['seen'] = x
        rec13['seen'] = x
        rec14['seen'] = x
        rec15['seen'] = x
        rec16['seen'] = x
        rec17['seen'] = x
        rec18['seen'] = x
        rec19['seen'] = x
        rec20['seen'] = x
        rec21['seen'] = x
        rec22['seen'] = x
        rec23['seen'] = x
        rec24['seen'] = x
        rec25['seen'] = x
        rec26['seen'] = x
        rec27['seen'] = x
        rec28['seen'] = x
        rec29['seen'] = x
        rec30['seen'] = x
        rec31['seen'] = x
        rec32['seen'] = x
        rec33['seen'] = x
        rec34['seen'] = x
        rec35['seen'] = x
        rec36['seen'] = x
        rec37['seen'] = x
        rec38['seen'] = x
        rec39['seen'] = x
        if x == 1:
            raise ValueError(x)
        if x == 2:
            raise KeyError(x)
        if x == 3:
            raise TypeError(x)
        if x == 4:
            raise OSError(x)
        yield x


def consume(xs):
    stage = "start"
    try:
        for v in items(xs):
            stage = v
    finally:
        REG["stage"] = stage
    return stage

consume(DATA[0:])
consume(DATA[1:])
consume(DATA[2:])
consume(DATA[3:])
consume(DATA[4:])
consume(DATA[5:])
consume(DATA[6:])
consume(DATA[7:])
consume(DATA[8:])
consume(DATA[9:])
consume(DATA[10:])
consume(DATA[11:])
