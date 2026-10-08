"""Properties computed from attributes, a setter that notifies observers with event dicts,
observers that keep their own state.

Expectations were written from the program text before running the engine."""


class Thermostat:
    def __init__(self, target):
        self._target = target
        self._observers = []
        self.readings = []

    @property
    def target(self):
        return self._target

    @target.setter
    def target(self, value):
        old = self._target
        self._target = value
        self._notify({"kind": "target", "old": old, "new": value})

    @property
    def summary(self):
        return {"target": self._target, "n": len(self.readings)}

    def subscribe(self, obs):
        self._observers.append(obs)

    def record(self, temp):
        self.readings.append({"temp": temp})
        if temp > self._target:
            self._notify({"kind": "over", "temp": temp, "by": temp - self._target})

    def _notify(self, event):
        for obs in self._observers:
            obs.on_event(event)


class Logger:
    def __init__(self):
        self.events = []

    def on_event(self, event):
        self.events.append(event)
        self.last = event


class Alarm:
    def __init__(self):
        self.triggered = False

    def on_event(self, event):
        if event["kind"] == "over":
            self.triggered = True
            self.detail = {"by": event["by"]}


t = Thermostat(20)
log = Logger()
alarm = Alarm()
t.subscribe(log)
t.subscribe(alarm)
t.target = 22
_ = 0                                     #@row log: present={events,last} definite={events,last} closed
last = log.last                           #@row last: present={kind,old,new} definite={kind,old,new} closed
# The target event never triggers the alarm: its kind is known.
_ = 0                                     #@row alarm: present={triggered} definite={triggered} closed
t.record(25)
# 25 > 22 is not evaluated: the "over" event is possible, and only it sets the detail.
_ = 0                                     #@row alarm: present={triggered,detail} definite={triggered} closed
last2 = log.last                          #@row last2: present={kind,old,new,temp,by} definite={kind} closed
s = t.summary                             #@row s: present={target,n} definite={target,n} closed
reading = t.readings[0]                   #@row reading: present={temp} definite={temp} closed
_ = 0                                     #@row t: present={_target,_observers,readings} definite={_target,_observers,readings} closed
alarm.detail["by"]                        #@ ok by
