"""`requests.packages`: `urllib3` as `requests` names it."""

import sys

import urllib3

sys.modules['requests.packages.urllib3'] = urllib3
for _name in list(sys.modules):
    if _name.startswith('urllib3.'):
        sys.modules['requests.packages.' + _name] = sys.modules[_name]
