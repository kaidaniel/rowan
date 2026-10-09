from . import register


@register("csv", sep=",")
class CsvIn:
    def read(self, text):
        rows = []
        for line in text.split("\n"):
            parts = line.split(",")
            rows.append({"a": parts[0], "b": parts[1]})
        return {"rows": rows, "format": self.plugin_name}
