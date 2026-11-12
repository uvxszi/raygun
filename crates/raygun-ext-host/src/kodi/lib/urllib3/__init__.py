"""`urllib3` for Kodi add-ons in Raygun (in place of `script.module.urllib3`): pools and requests
answered by Raygun's HTTP client."""

import json as _json

import _raygun_http
from . import exceptions, util
from .util.retry import Retry
from .util.timeout import Timeout
from .util.url import parse_url

__version__ = '1.26.18'


class HTTPHeaderDict(dict):
    def get(self, key, default=None):
        for name, value in self.items():
            if name.lower() == key.lower():
                return value
        return default

    def getlist(self, key):
        value = self.get(key)
        return [] if value is None else [value]


class HTTPResponse:
    def __init__(self, status, url, headers, data):
        self.status = status
        self.reason = ''
        self.headers = HTTPHeaderDict()
        for name, value in headers:
            if name in self.headers:
                self.headers[name] += ', ' + value
            else:
                self.headers[name] = value
        self.data = data
        self._url = url
        self._position = 0

    @property
    def url(self):
        return self._url

    def geturl(self):
        return self._url

    def read(self, amt=None, decode_content=None, cache_content=False):
        if amt is None:
            chunk = self.data[self._position:]
        else:
            chunk = self.data[self._position:self._position + amt]
        self._position += len(chunk)
        return chunk

    def stream(self, amt=65536, decode_content=None):
        while True:
            chunk = self.read(amt)
            if not chunk:
                break
            yield chunk

    def json(self):
        return _json.loads(self.data.decode('utf-8'))

    def getheader(self, name, default=None):
        return self.headers.get(name, default)

    def getheaders(self):
        return self.headers

    def info(self):
        return self.headers

    def release_conn(self):
        pass

    def close(self):
        pass

    def drain_conn(self):
        pass


def _request(method, url, fields=None, headers=None, body=None, timeout=None, redirect=True,
             **kwargs):
    import urllib.parse
    if fields:
        if method.upper() in ('GET', 'HEAD', 'DELETE', 'OPTIONS'):
            url += ('&' if '?' in url else '?') + urllib.parse.urlencode(fields)
        else:
            body = urllib.parse.urlencode(fields)
            headers = dict(headers or {})
            headers.setdefault('Content-Type', 'application/x-www-form-urlencoded')
    if isinstance(timeout, Timeout):
        timeout = timeout.total or timeout.read_timeout
    try:
        status, final_url, answer, data = _raygun_http.fetch(
            method, url, headers or {}, body, timeout, bool(redirect))
    except TimeoutError as error:
        raise exceptions.ReadTimeoutError(None, url, str(error))
    except OSError as error:
        raise exceptions.MaxRetryError(None, url, error)
    return HTTPResponse(status, final_url, answer, data)


class RequestMethods:
    def __init__(self, headers=None):
        self.headers = headers or {}

    def request(self, method, url, fields=None, headers=None, **kwargs):
        merged = dict(self.headers)
        merged.update(headers or {})
        return _request(method, url, fields=fields, headers=merged, **kwargs)

    def urlopen(self, method, url, body=None, headers=None, **kwargs):
        merged = dict(self.headers)
        merged.update(headers or {})
        return _request(method, url, headers=merged, body=body, **kwargs)

    def request_encode_url(self, method, url, fields=None, headers=None, **kwargs):
        return self.request(method, url, fields=fields, headers=headers, **kwargs)

    def request_encode_body(self, method, url, fields=None, headers=None, **kwargs):
        return self.request(method, url, fields=fields, headers=headers, **kwargs)


class PoolManager(RequestMethods):
    def __init__(self, num_pools=10, headers=None, **kwargs):
        super().__init__(headers)

    def clear(self):
        pass

    def connection_from_url(self, url, pool_kwargs=None):
        return self


class ProxyManager(PoolManager):
    def __init__(self, proxy_url, num_pools=10, headers=None, **kwargs):
        super().__init__(num_pools, headers)


class HTTPConnectionPool(PoolManager):
    def __init__(self, host, port=None, **kwargs):
        super().__init__()


HTTPSConnectionPool = HTTPConnectionPool


def proxy_from_url(url, **kwargs):
    return ProxyManager(url, **kwargs)


def connection_from_url(url, **kwargs):
    return HTTPConnectionPool(url)


def disable_warnings(category=None):
    pass


def add_stderr_logger(level=None):
    pass


def request(method, url, **kwargs):
    return _request(method, url, **kwargs)
