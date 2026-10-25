"""`date.isocalendar()` gives an `IsoCalendarDate`, a named tuple: its text is
`datetime.IsoCalendarDate(year=2024, week=5, weekday=3)` and it has `.week`. (Before, it was
a plain tuple, whose text is `(2024, 5, 3)`.)

Expectations were written from the program text before running the engine."""
import datetime

cal = datetime.date(2024, 1, 31).isocalendar()
text = repr(cal)
#@value text: includes={"datetime.IsoCalendarDate(year=2024, week=5, weekday=3)"}
