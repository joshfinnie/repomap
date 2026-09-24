import json


class Widget:
    @property
    def label(self):
        return "widget"

    def render(self, indent=0):
        return json.dumps({"label": self.label})


def _private_helper():
    pass
