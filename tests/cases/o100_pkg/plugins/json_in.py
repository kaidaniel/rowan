from . import register


@register("json", strict=True)
class JsonIn:
    def read(self, text):
        return {"rows": [], "format": self.plugin_name, "strict": True}
