"""HTTP for Kodi add-ons in Raygun. Every request goes through `_raygun.fetch`: Raygun's own
client, which reaches public hosts only and honours Offline Mode. `http.client`'s connections are
replaced by `Connection`, so `urllib.request` and whatever builds on it work unchanged."""

import io

import _raygun

DEFAULT_TIMEOUT = 20.0


class FetchError(OSError):
    pass


def fetch(method, url, headers=(), body=None, timeout=None, follow=True):
    """One request: `(status, final_url, [(name, value)...], body)`."""
    if timeout is None or not isinstance(timeout, (int, float)):
        timeout = DEFAULT_TIMEOUT
    if isinstance(body, str):
        body = body.encode('utf-8')
    elif body is not None and not isinstance(body, bytes):
        body = bytes(body)
    pairs = []
    for name, value in (headers.items() if hasattr(headers, 'items') else headers):
        if value is None:
            continue
        if isinstance(name, bytes):
            name = name.decode('latin-1')
        if isinstance(value, bytes):
            value = value.decode('latin-1')
        pairs.append((str(name), str(value)))
    return _raygun.fetch(method.upper(), url, pairs, body, float(timeout), bool(follow))


class _Headers(list):
    def get(self, name, default=None):
        name = name.lower()
        for key, value in self:
            if key.lower() == name:
                return value
        return default


class Response(io.BufferedIOBase):
    """What `Connection.getresponse` answers: `http.client.HTTPResponse`'s interface over a body
    already read."""

    def __init__(self, status, url, headers, body, method):
        import http.client
        super().__init__()
        self.status = status
        self.code = status
        self.url = url
        self.version = 11
        self.reason = http.client.responses.get(status, '')
        message = http.client.HTTPMessage()
        for name, value in headers:
            message[name] = value
        self.msg = message
        self.headers = message
        self.chunked = False
        self.will_close = True
        self._method = method
        self._body = io.BytesIO(body if method != 'HEAD' else b'')
        self.length = len(body)
        self.fp = self._body

    def read(self, amt=None):
        if amt is None or amt < 0:
            data = self._body.read()
        else:
            data = self._body.read(amt)
        self.length = max(0, self.length - len(data))
        return data

    def read1(self, amt=-1):
        return self.read(amt)

    def readinto(self, buffer):
        data = self.read(len(buffer))
        buffer[:len(data)] = data
        return len(data)

    def readline(self, limit=-1):
        return self._body.readline(limit)

    def peek(self, n=0):
        position = self._body.tell()
        data = self._body.read(max(n, 1))
        self._body.seek(position)
        return data

    def readable(self):
        return True

    def getheader(self, name, default=None):
        values = self.headers.get_all(name)
        if values is None:
            return default
        return ', '.join(values)

    def getheaders(self):
        return list(self.headers.items())

    def info(self):
        return self.headers

    def geturl(self):
        return self.url

    def getcode(self):
        return self.status

    def isclosed(self):
        return self._body.closed

    def fileno(self):
        raise io.UnsupportedOperation('fileno')

    def close(self):
        super().close()

    def __iter__(self):
        return iter(self._body.readlines())


class Connection:
    """`http.client.HTTPConnection` and `HTTPSConnection`, answered by `_raygun.fetch`."""

    default_port = 80
    scheme = 'http'
    response_class = Response
    debuglevel = 0
    _http_vsn = 11
    _http_vsn_str = 'HTTP/1.1'
    auto_open = 1

    def __init__(self, host, port=None, timeout=None, source_address=None, blocksize=8192,
                 context=None, **kwargs):
        self.host = host
        self.port = port
        self.timeout = timeout if isinstance(timeout, (int, float)) else DEFAULT_TIMEOUT
        self._method = 'GET'
        self._url = '/'
        self._headers = []
        self._body = b''
        self._tunnel = None
        self.sock = None

    def set_debuglevel(self, level):
        pass

    def set_tunnel(self, host, port=None, headers=None):
        self._tunnel = (host, port)

    def connect(self):
        pass

    def close(self):
        pass

    def putrequest(self, method, url, skip_host=False, skip_accept_encoding=False):
        self._method = method
        self._url = url
        self._headers = []
        self._body = b''

    def putheader(self, header, *values):
        if isinstance(header, bytes):
            header = header.decode('latin-1')
        text = ', '.join(v.decode('latin-1') if isinstance(v, bytes) else str(v) for v in values)
        self._headers.append((header, text))

    def endheaders(self, message_body=None, *, encode_chunked=False):
        if message_body is not None:
            self.send(message_body)

    def send(self, data):
        if hasattr(data, 'read'):
            data = data.read()
        if isinstance(data, str):
            data = data.encode('iso-8859-1')
        elif not isinstance(data, bytes):
            data = b''.join(chunk if isinstance(chunk, bytes) else str(chunk).encode()
                            for chunk in data)
        self._body += data

    def request(self, method, url, body=None, headers={}, *, encode_chunked=False):
        self.putrequest(method, url)
        for name, value in headers.items():
            self.putheader(name, value)
        if body is not None:
            self.send(body)

    def _full_url(self):
        if self._url.startswith(('http://', 'https://')):
            return self._url
        host = self.host
        if self.port and self.port != self.default_port:
            host = '%s:%s' % (host, self.port)
        return '%s://%s%s' % (self.scheme, host, self._url)

    def getresponse(self):
        method = self._method
        headers = [(name, value) for name, value in self._headers
                   if name.lower() not in ('host', 'content-length', 'connection')]
        body = self._body if (self._body or method in ('POST', 'PUT', 'PATCH')) else None
        status, final_url, answer_headers, data = fetch(
            method, self._full_url(), headers, body, self.timeout, False)
        return Response(status, final_url, answer_headers, data, method)


class SecureConnection(Connection):
    default_port = 443
    scheme = 'https'


def patch_http_client(module):
    """Swaps `http.client`'s connections for Raygun's (done as the module is imported)."""
    class HTTPConnection(Connection):
        response_class = module.HTTPResponse

    class HTTPSConnection(SecureConnection):
        response_class = module.HTTPResponse

    HTTPConnection.response_class = Response
    HTTPSConnection.response_class = Response
    module.HTTPConnection = HTTPConnection
    module.HTTPSConnection = HTTPSConnection
    if 'HTTPSConnection' not in module.__all__:
        module.__all__.append('HTTPSConnection')
