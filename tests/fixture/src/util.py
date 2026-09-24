import json


class Widget:
    @property
    def label(self):
        return "widget"

    def render(self, indent=0):
        payload = {"label": self.label}
        return json.dumps(payload)


def _private_helper():
    pass
