"""`cgi`, gone from Python 3.13, for older add-ons: the helpers they use."""

import html
import urllib.parse

__all__ = ['escape', 'parse_qs', 'parse_qsl', 'parse_header', 'FieldStorage']


def escape(s, quote=False):
    return html.escape(s, quote=quote)


def parse_qs(qs, keep_blank_values=False, strict_parsing=False):
    return urllib.parse.parse_qs(qs, keep_blank_values, strict_parsing)


def parse_qsl(qs, keep_blank_values=False, strict_parsing=False):
    return urllib.parse.parse_qsl(qs, keep_blank_values, strict_parsing)


def parse_header(line):
    parts = [part.strip() for part in line.split(';')]
    key = parts.pop(0).lower() if parts else ''
    params = {}
    for part in parts:
        name, _, value = part.partition('=')
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] == '"':
            value = value[1:-1]
        params[name.strip().lower()] = value
    return key, params


class FieldStorage:
    def __init__(self, *args, **kwargs):
        self.list = []

    def getvalue(self, key, default=None):
        return default

    def keys(self):
        return []
