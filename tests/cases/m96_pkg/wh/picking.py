"""Pick planning: strategies registered by a decorator, waves of orders, pick lists grouped
by zone, packing picks into boxes under weight limits, and a planner that records what it
did."""
from collections import defaultdict

STRATEGIES = {}


def strategy(name):
    def deco(cls):
        STRATEGIES[name] = cls
        cls.strategy_name = name
        return cls

    return deco


class Strategy:
    strategy_name = "base"

    def __init__(self, planner):
        self.planner = planner

    def order_lines(self, lines):
        return list(lines)

    def describe(self):
        return {"name": self.strategy_name}


@strategy("fifo")
class Fifo(Strategy):
    def order_lines(self, lines):
        return sorted(lines, key=lambda line: line["sku"])


@strategy("nearest")
class Nearest(Strategy):
    def order_lines(self, lines):
        return sorted(lines, key=lambda line: line.get("zone", "Z"))

    def describe(self):
        d = super().describe()
        d["walks"] = self.planner.walks
        return d


@strategy("zone")
class ByZone(Strategy):
    def order_lines(self, lines):
        grouped = defaultdict(list)
        for line in lines:
            grouped[line.get("zone", "?")].append(line)
        out = []
        for zone in sorted(grouped):
            out.extend(grouped[zone])
        return out


class Planner:
    def __init__(self, inventory, cfg):
        self.inventory = inventory
        self.cfg = cfg
        self.walks = 0
        self.waves = []
        self.shortages = []
        name = cfg.get("picking.strategy", "fifo")
        cls = STRATEGIES.get(name, Fifo)
        self.strategy = cls(self)

    def line_with_zone(self, line):
        where = self.inventory.locate(line["sku"])
        out = dict(line)
        if where:
            out["zone"] = self.inventory.locations[sorted(where)[0]].zone
        return out

    def plan_order(self, order):
        lines = [self.line_with_zone(line) for line in order["lines"]]
        ordered = self.strategy.order_lines(lines)
        picks = []
        for line in ordered:
            result = self.inventory.consume(line["sku"], line["qty"], "order " + str(order["id"]))
            for pick in result["picks"]:
                picks.append({"sku": line["sku"], "lot": pick["lot"], "qty": pick["qty"], "loc": pick["loc"]})
            if result["short"]:
                self.shortages.append({"order": order["id"], "sku": line["sku"], "short": result["short"]})
        self.walks += 1
        plan = {"order": order["id"], "picks": picks, "lines": len(ordered)}
        if order.get("rush"):
            plan["rush"] = True
        return plan

    def plan_wave(self, orders):
        size = self.cfg.get("picking.wave_size", 3)
        wave = {"n": 0, "plans": [], "by_zone": defaultdict(list)}
        for order in orders[:size]:
            plan = self.plan_order(order)
            wave["plans"].append(plan)
            wave["n"] += 1
            for pick in plan["picks"]:
                zone = self.inventory.locations[pick["loc"]].zone
                wave["by_zone"][zone].append(pick)
        wave["by_zone"] = dict(wave["by_zone"])
        self.waves.append(wave)
        return wave

    def pack(self, plan):
        limit = self.cfg.get("limits.box_weight_g", 10000)
        boxes = []
        box = {"n": 1, "items": [], "weight_g": 0}
        for pick in plan["picks"]:
            product = self.inventory.products[pick["sku"]]
            w = product.weight_g * pick["qty"]
            if box["items"] and box["weight_g"] + w > limit:
                boxes.append(box)
                box = {"n": len(boxes) + 1, "items": [], "weight_g": 0}
            box["items"].append({"sku": pick["sku"], "qty": pick["qty"]})
            box["weight_g"] += w
        if box["items"]:
            boxes.append(box)
        return boxes

    def describe(self):
        return {"strategy": self.strategy.describe(), "waves": len(self.waves), "walks": self.walks, "shortages": len(self.shortages)}
