"""`requests.compat`."""

import json  # noqa: F401
from collections.abc import Callable, Mapping, MutableMapping  # noqa: F401
from http import cookiejar as cookielib  # noqa: F401
from http.cookies import Morsel  # noqa: F401
from io import StringIO  # noqa: F401
from urllib.parse import (  # noqa: F401
    quote, quote_plus, unquote, unquote_plus, urldefrag, urlencode, urljoin, urlparse, urlsplit,
    urlunparse,
)
from urllib.request import getproxies, parse_http_list, proxy_bypass  # noqa: F401

is_py2 = False
is_py3 = True
builtin_str = str
str = str
bytes = bytes
basestring = (str, bytes)
numeric_types = (int, float)
integer_types = (int,)
