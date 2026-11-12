"""`requests.cookies`: a session's cookies, kept in Python as `requests` keeps them."""

import time
import urllib.parse


class Cookie:
    def __init__(self, name, value, domain='', path='/', expires=None, secure=False):
        self.name = name
        self.value = value
        self.domain = domain
        self.path = path or '/'
        self.expires = expires
        self.secure = secure
        self.version = 0
        self.port = None
        self.discard = expires is None
        self.comment = None
        self.rest = {}

    def is_expired(self, now=None):
        return self.expires is not None and self.expires <= (now or time.time())

    def has_nonstandard_attr(self, name):
        return name in self.rest

    def __repr__(self):
        return '<Cookie %s=%s for %s%s>' % (self.name, self.value, self.domain, self.path)


class CookieConflictError(RuntimeError):
    pass


class RequestsCookieJar:
    def __init__(self, policy=None):
        self._cookies = []
        self._policy = policy

    def set_policy(self, policy):
        self._policy = policy

    def __iter__(self):
        now = time.time()
        return iter([cookie for cookie in self._cookies if not cookie.is_expired(now)])

    def __len__(self):
        return len(list(iter(self)))

    def __contains__(self, name):
        return any(cookie.name == name for cookie in self)

    def __getitem__(self, name):
        for cookie in self:
            if cookie.name == name:
                return cookie.value
        raise KeyError(name)

    def __setitem__(self, name, value):
        self.set(name, value)

    def __delitem__(self, name):
        self._cookies = [cookie for cookie in self._cookies if cookie.name != name]

    def get(self, name, default=None, domain=None, path=None):
        for cookie in self:
            if cookie.name == name and (domain is None or cookie.domain == domain):
                return cookie.value
        return default

    def set(self, name, value, **kwargs):
        if value is None:
            self.__delitem__(name)
            return None
        domain = kwargs.get('domain', '') or ''
        path = kwargs.get('path', '/') or '/'
        self._cookies = [cookie for cookie in self._cookies
                         if not (cookie.name == name and cookie.domain == domain
                                 and cookie.path == path)]
        cookie = Cookie(name, str(value), domain, path, kwargs.get('expires'),
                        kwargs.get('secure', False))
        self._cookies.append(cookie)
        return cookie

    def set_cookie(self, cookie, *args, **kwargs):
        self.set(cookie.name, cookie.value, domain=getattr(cookie, 'domain', ''),
                 path=getattr(cookie, 'path', '/'), expires=getattr(cookie, 'expires', None),
                 secure=getattr(cookie, 'secure', False))

    def update(self, other):
        if other is None:
            return
        if isinstance(other, RequestsCookieJar):
            for cookie in other._cookies:
                self.set_cookie(cookie)
        elif hasattr(other, 'items'):
            for name, value in other.items():
                self.set(name, value)
        else:
            for cookie in other:
                self.set_cookie(cookie)

    def keys(self):
        return [cookie.name for cookie in self]

    def values(self):
        return [cookie.value for cookie in self]

    def items(self):
        return [(cookie.name, cookie.value) for cookie in self]

    def get_dict(self, domain=None, path=None):
        return {cookie.name: cookie.value for cookie in self
                if domain is None or cookie.domain == domain}

    def list_domains(self):
        return sorted({cookie.domain for cookie in self})

    def clear(self, domain=None, path=None, name=None):
        self._cookies = [cookie for cookie in self._cookies
                         if not ((domain is None or cookie.domain == domain)
                                 and (name is None or cookie.name == name))]

    def clear_session_cookies(self):
        self._cookies = [cookie for cookie in self._cookies if cookie.expires is not None]

    def copy(self):
        other = RequestsCookieJar()
        other.update(self)
        return other

    def _take(self, line, url):
        """Keeps the cookie a `Set-Cookie` line sets for `url`."""
        parts = [part.strip() for part in line.split(';')]
        if not parts or '=' not in parts[0]:
            return
        name, value = parts[0].split('=', 1)
        host = urllib.parse.urlsplit(url).hostname or ''
        domain, path, expires, secure = host, '/', None, False
        for part in parts[1:]:
            key, _, attr = part.partition('=')
            key = key.strip().lower()
            if key == 'domain' and attr:
                domain = attr.strip().lstrip('.')
            elif key == 'path' and attr:
                path = attr.strip()
            elif key == 'max-age':
                try:
                    expires = time.time() + int(attr)
                except ValueError:
                    pass
            elif key == 'expires' and expires is None:
                try:
                    import email.utils
                    expires = email.utils.parsedate_to_datetime(attr).timestamp()
                except Exception:
                    pass
            elif key == 'secure':
                secure = True
        self.set(name.strip(), value.strip(), domain=domain, path=path, expires=expires,
                 secure=secure)

    def _header_for(self, url):
        """The `Cookie` header this jar sends to `url`."""
        parts = urllib.parse.urlsplit(url)
        host = (parts.hostname or '').lower()
        path = parts.path or '/'
        found = []
        for cookie in self:
            domain = (cookie.domain or '').lower()
            if domain and not (host == domain or host.endswith('.' + domain)):
                continue
            if not path.startswith(cookie.path or '/'):
                continue
            if cookie.secure and parts.scheme != 'https':
                continue
            found.append('%s=%s' % (cookie.name, cookie.value))
        return '; '.join(found)


def cookiejar_from_dict(cookie_dict, cookiejar=None, overwrite=True):
    jar = cookiejar if cookiejar is not None else RequestsCookieJar()
    for name, value in (cookie_dict or {}).items():
        if overwrite or name not in jar:
            jar.set(name, value)
    return jar


def create_cookie(name, value, **kwargs):
    return Cookie(name, value, kwargs.get('domain', ''), kwargs.get('path', '/'),
                  kwargs.get('expires'), kwargs.get('secure', False))


def merge_cookies(cookiejar, cookies):
    if hasattr(cookies, 'items') and not isinstance(cookies, RequestsCookieJar):
        return cookiejar_from_dict(cookies, cookiejar)
    cookiejar.update(cookies)
    return cookiejar


def dict_from_cookiejar(cj):
    return {cookie.name: cookie.value for cookie in cj}


def add_dict_to_cookiejar(cj, cookie_dict):
    return cookiejar_from_dict(cookie_dict, cj)
