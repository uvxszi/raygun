"""`requests` for Kodi add-ons in Raygun (in place of `script.module.requests`): the library's
interface over Raygun's HTTP client (`_raygun_http.fetch`). Sessions keep their own cookies, as
`requests` does; certificates are always checked by Raygun, whatever `verify` says."""

import base64
import json as _json
import time
import urllib.parse

import _raygun_http
from . import exceptions
from .exceptions import (
    ConnectionError, ConnectTimeout, HTTPError, InvalidURL, JSONDecodeError, MissingSchema,
    ReadTimeout, RequestException, SSLError, Timeout, TooManyRedirects,
)
from .structures import CaseInsensitiveDict
from .cookies import RequestsCookieJar, cookiejar_from_dict
from .status_codes import codes

__version__ = '2.31.0'
__build__ = 0x023100
__title__ = 'requests'
__author__ = 'Kenneth Reitz'
__license__ = 'Apache-2.0'
__copyright__ = 'Copyright Kenneth Reitz'
_DEFAULT_AGENT = 'Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0 Mobile Safari/537.36'
_MAX_REDIRECTS = 30


def _encode_params(params):
    if params is None:
        return ''
    if isinstance(params, (str, bytes)):
        return params.decode() if isinstance(params, bytes) else params
    if hasattr(params, 'items'):
        params = params.items()
    pairs = []
    for key, value in params:
        if value is None:
            continue
        if isinstance(value, (list, tuple)):
            for item in value:
                pairs.append((key, item))
        else:
            pairs.append((key, value))
    return urllib.parse.urlencode(pairs)


def _join_query(url, query):
    if not query:
        return url
    head, sep, fragment = url.partition('#')
    head = head + ('&' if '?' in head else '?') + query
    return head + (sep + fragment if sep else '')


class Request:
    def __init__(self, method=None, url=None, headers=None, files=None, data=None, params=None,
                 auth=None, cookies=None, hooks=None, json=None):
        self.method = method
        self.url = url
        self.headers = headers or {}
        self.files = files
        self.data = data
        self.params = params
        self.auth = auth
        self.cookies = cookies
        self.json = json

    def prepare(self):
        prepared = PreparedRequest()
        prepared.prepare(method=self.method, url=self.url, headers=self.headers, data=self.data,
                         params=self.params, auth=self.auth, cookies=self.cookies, json=self.json,
                         files=self.files)
        return prepared


class PreparedRequest:
    def __init__(self):
        self.method = None
        self.url = None
        self.headers = CaseInsensitiveDict()
        self.body = None
        self._cookies = None

    def prepare(self, method=None, url=None, headers=None, files=None, data=None, params=None,
                auth=None, cookies=None, hooks=None, json=None):
        self.method = (method or 'GET').upper()
        if isinstance(url, bytes):
            url = url.decode('utf-8')
        url = str(url).strip()
        if '://' not in url:
            raise MissingSchema('Invalid URL %r: No scheme supplied.' % url)
        scheme = url.split('://', 1)[0].lower()
        if scheme not in ('http', 'https'):
            raise InvalidURL('Raygun fetches only http and https addresses: %r' % url)
        self.url = _join_query(url, _encode_params(params))
        self.headers = CaseInsensitiveDict(headers or {})
        body = None
        if files:
            body, content_type = _multipart(data, files)
            self.headers.setdefault('Content-Type', content_type)
        elif json is not None:
            body = _json.dumps(json).encode('utf-8')
            self.headers.setdefault('Content-Type', 'application/json')
        elif data is not None and data != {} and data != []:
            if isinstance(data, (str, bytes)):
                body = data
            elif hasattr(data, 'read'):
                body = data.read()
            else:
                body = _encode_params(data)
                self.headers.setdefault('Content-Type', 'application/x-www-form-urlencoded')
        if isinstance(body, str):
            body = body.encode('utf-8')
        self.body = body
        if auth is not None:
            if isinstance(auth, tuple) and len(auth) == 2:
                token = base64.b64encode(
                    ('%s:%s' % auth).encode('latin-1')).decode('ascii')
                self.headers['Authorization'] = 'Basic ' + token
            elif callable(auth):
                auth(self)
        if cookies:
            jar = cookies if isinstance(cookies, RequestsCookieJar) else cookiejar_from_dict(cookies)
            self._cookies = jar

    def copy(self):
        other = PreparedRequest()
        other.method = self.method
        other.url = self.url
        other.headers = CaseInsensitiveDict(self.headers)
        other.body = self.body
        other._cookies = self._cookies
        return other

    @property
    def path_url(self):
        parts = urllib.parse.urlsplit(self.url)
        return (parts.path or '/') + ('?' + parts.query if parts.query else '')


def _multipart(data, files):
    boundary = 'raygun%d' % int(time.time() * 1000)
    lines = []
    fields = _encode_params(data) if isinstance(data, (str, bytes)) else None
    for key, value in (data.items() if hasattr(data, 'items') else (data or [])):
        lines.append(('--%s\r\nContent-Disposition: form-data; name="%s"\r\n\r\n%s\r\n'
                      % (boundary, key, value)).encode('utf-8'))
    for key, value in (files.items() if hasattr(files, 'items') else files):
        filename, content, kind = key, value, 'application/octet-stream'
        if isinstance(value, (tuple, list)):
            filename = value[0]
            content = value[1]
            if len(value) > 2:
                kind = value[2]
        if hasattr(content, 'read'):
            content = content.read()
        if isinstance(content, str):
            content = content.encode('utf-8')
        lines.append(('--%s\r\nContent-Disposition: form-data; name="%s"; filename="%s"\r\n'
                      'Content-Type: %s\r\n\r\n' % (boundary, key, filename, kind)).encode('utf-8')
                     + content + b'\r\n')
    lines.append(('--%s--\r\n' % boundary).encode('utf-8'))
    del fields
    return b''.join(lines), 'multipart/form-data; boundary=' + boundary


class Response:
    def __init__(self):
        self.status_code = None
        self.headers = CaseInsensitiveDict()
        self.url = None
        self.encoding = None
        self.history = []
        self.reason = ''
        self.cookies = RequestsCookieJar()
        self.elapsed = None
        self.request = None
        self.raw = None
        self._content = b''
        self.connection = None

    def __repr__(self):
        return '<Response [%s]>' % self.status_code

    def __bool__(self):
        return self.ok

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    def __iter__(self):
        return self.iter_content(128)

    @property
    def ok(self):
        try:
            self.raise_for_status()
        except HTTPError:
            return False
        return True

    @property
    def is_redirect(self):
        return 'location' in self.headers and self.status_code in (301, 302, 303, 307, 308)

    @property
    def is_permanent_redirect(self):
        return 'location' in self.headers and self.status_code in (301, 308)

    @property
    def content(self):
        return self._content

    @property
    def apparent_encoding(self):
        try:
            self._content.decode('utf-8')
            return 'utf-8'
        except UnicodeDecodeError:
            return 'ISO-8859-1'

    @property
    def text(self):
        encoding = self.encoding or self.apparent_encoding
        try:
            return self._content.decode(encoding, errors='replace')
        except LookupError:
            return self._content.decode('utf-8', errors='replace')

    @property
    def links(self):
        return {}

    @property
    def next(self):
        return None

    def json(self, **kwargs):
        text = self.text
        try:
            return _json.loads(text, **kwargs)
        except ValueError as error:
            raise JSONDecodeError(str(error), text, 0)

    def iter_content(self, chunk_size=1, decode_unicode=False):
        data = self._content
        size = chunk_size or len(data) or 1
        for start in range(0, len(data), size):
            chunk = data[start:start + size]
            yield chunk.decode(self.encoding or 'utf-8', 'replace') if decode_unicode else chunk

    def iter_lines(self, chunk_size=512, decode_unicode=False, delimiter=None):
        text = self.text if decode_unicode else self._content
        parts = text.split(delimiter) if delimiter else text.splitlines()
        for line in parts:
            yield line

    def raise_for_status(self):
        if self.status_code is None:
            return
        if 400 <= self.status_code < 500:
            kind = 'Client Error'
        elif 500 <= self.status_code < 600:
            kind = 'Server Error'
        else:
            return
        raise HTTPError('%s %s: %s for url: %s' % (self.status_code, kind, self.reason, self.url),
                        response=self)

    def close(self):
        pass


def _encoding_of(content_type):
    if not content_type:
        return None
    for part in content_type.split(';')[1:]:
        key, _, value = part.strip().partition('=')
        if key.lower() == 'charset':
            return value.strip('"\' ') or None
    if content_type.lower().startswith('text/'):
        return 'ISO-8859-1'
    return None


class Session:
    def __init__(self):
        self.headers = CaseInsensitiveDict({
            'User-Agent': _DEFAULT_AGENT,
            'Accept-Encoding': 'gzip, deflate',
            'Accept': '*/*',
            'Connection': 'keep-alive',
        })
        self.cookies = RequestsCookieJar()
        self.auth = None
        self.proxies = {}
        self.params = {}
        self.verify = True
        self.cert = None
        self.stream = False
        self.trust_env = True
        self.max_redirects = _MAX_REDIRECTS
        self.adapters = {}
        self.hooks = {'response': []}

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    def close(self):
        pass

    def mount(self, prefix, adapter):
        self.adapters[prefix] = adapter

    def get_adapter(self, url):
        from .adapters import HTTPAdapter
        return HTTPAdapter()

    def prepare_request(self, request):
        headers = CaseInsensitiveDict(self.headers)
        headers.update(request.headers or {})
        params = dict(self.params)
        prepared = PreparedRequest()
        prepared.prepare(method=request.method, url=request.url, headers=headers,
                         data=request.data, params=_merge_params(params, request.params),
                         auth=request.auth or self.auth, cookies=request.cookies,
                         json=request.json, files=request.files)
        return prepared

    def request(self, method, url, params=None, data=None, headers=None, cookies=None,
                files=None, auth=None, timeout=None, allow_redirects=True, proxies=None,
                hooks=None, stream=None, verify=None, cert=None, json=None):
        request = Request(method=method, url=url, headers=headers or {}, files=files, data=data,
                          params=params, auth=auth, cookies=cookies, json=json)
        prepared = self.prepare_request(request)
        return self.send(prepared, timeout=timeout, allow_redirects=allow_redirects)

    def send(self, request, timeout=None, allow_redirects=True, **kwargs):
        if isinstance(timeout, tuple):
            timeout = sum(value for value in timeout if value)
        history = []
        method = request.method
        url = request.url
        body = request.body
        headers = CaseInsensitiveDict(request.headers)
        extra = request._cookies
        started = time.time()
        while True:
            jar = RequestsCookieJar()
            jar.update(self.cookies)
            if extra is not None:
                jar.update(extra)
            cookie = jar._header_for(url)
            send_headers = [(key, value) for key, value in headers.items()
                            if key.lower() != 'cookie']
            if cookie:
                send_headers.append(('Cookie', cookie))
            elif 'cookie' in headers:
                send_headers.append(('Cookie', headers['cookie']))
            try:
                status, final_url, answer_headers, content = _raygun_http.fetch(
                    method, url, send_headers, body, timeout, False)
            except TimeoutError as error:
                raise ReadTimeout(str(error), request=request)
            except _raygun_http.FetchError as error:
                raise ConnectionError(str(error), request=request)
            except OSError as error:
                raise ConnectionError(str(error), request=request)
            response = Response()
            response.status_code = status
            response.url = final_url
            response.request = request
            response.reason = _reason(status)
            response._content = content
            merged = CaseInsensitiveDict()
            for key, value in answer_headers:
                if key.lower() == 'set-cookie':
                    response.cookies._take(value, final_url)
                    self.cookies._take(value, final_url)
                if key in merged:
                    merged[key] = merged[key] + ', ' + value
                else:
                    merged[key] = value
            response.headers = merged
            response._content = _decoded(content, merged.get('content-encoding'))
            response.encoding = _encoding_of(merged.get('content-type'))
            location = merged.get('location')
            if (allow_redirects and location and status in (301, 302, 303, 307, 308)
                    and len(history) < self.max_redirects):
                history.append(response)
                url = urllib.parse.urljoin(url, location)
                if status == 303 or (status in (301, 302) and method == 'POST'):
                    method = 'GET'
                    body = None
                    headers.pop('content-type', None)
                continue
            if allow_redirects and location and len(history) >= self.max_redirects:
                raise TooManyRedirects('Exceeded %d redirects.' % self.max_redirects,
                                       response=response)
            response.history = history
            import datetime
            response.elapsed = datetime.timedelta(seconds=time.time() - started)
            for hook in self.hooks.get('response', []):
                hook(response)
            return response

    def get(self, url, **kwargs):
        kwargs.setdefault('allow_redirects', True)
        return self.request('GET', url, **kwargs)

    def options(self, url, **kwargs):
        return self.request('OPTIONS', url, **kwargs)

    def head(self, url, **kwargs):
        kwargs.setdefault('allow_redirects', False)
        return self.request('HEAD', url, **kwargs)

    def post(self, url, data=None, json=None, **kwargs):
        return self.request('POST', url, data=data, json=json, **kwargs)

    def put(self, url, data=None, **kwargs):
        return self.request('PUT', url, data=data, **kwargs)

    def patch(self, url, data=None, **kwargs):
        return self.request('PATCH', url, data=data, **kwargs)

    def delete(self, url, **kwargs):
        return self.request('DELETE', url, **kwargs)


def _merge_params(base, extra):
    if not base:
        return extra
    if not extra:
        return base
    if isinstance(extra, (str, bytes)):
        return _encode_params(base) + '&' + (extra.decode() if isinstance(extra, bytes) else extra)
    merged = dict(base)
    merged.update(dict(extra.items() if hasattr(extra, 'items') else extra))
    return merged


def _decoded(content, encoding):
    """The body as sent before compression, as `requests` gives it."""
    if not content or not encoding:
        return content
    import zlib
    encoding = encoding.lower()
    try:
        if 'gzip' in encoding:
            return zlib.decompress(content, 16 + zlib.MAX_WBITS)
        if 'deflate' in encoding:
            try:
                return zlib.decompress(content)
            except zlib.error:
                return zlib.decompress(content, -zlib.MAX_WBITS)
    except zlib.error:
        return content
    return content


def _reason(status):
    import http.client
    return http.client.responses.get(status, '')


def session():
    return Session()


def request(method, url, **kwargs):
    with Session() as current:
        return current.request(method=method, url=url, **kwargs)


def get(url, params=None, **kwargs):
    return request('get', url, params=params, **kwargs)


def options(url, **kwargs):
    return request('options', url, **kwargs)


def head(url, **kwargs):
    kwargs.setdefault('allow_redirects', False)
    return request('head', url, **kwargs)


def post(url, data=None, json=None, **kwargs):
    return request('post', url, data=data, json=json, **kwargs)


def put(url, data=None, **kwargs):
    return request('put', url, data=data, **kwargs)


def patch(url, data=None, **kwargs):
    return request('patch', url, data=data, **kwargs)


def delete(url, **kwargs):
    return request('delete', url, **kwargs)


from . import adapters, auth, compat, hooks, models, packages, sessions, utils  # noqa: E402
