"""`urllib3.util.url`."""

import collections
import urllib.parse

Url = collections.namedtuple('Url', ['scheme', 'auth', 'host', 'port', 'path', 'query', 'fragment'])


def parse_url(url):
    parts = urllib.parse.urlsplit(url)
    auth = None
    if parts.username:
        auth = parts.username + (':' + parts.password if parts.password else '')
    return Url(parts.scheme or None, auth, parts.hostname, parts.port, parts.path or None,
               parts.query or None, parts.fragment or None)
