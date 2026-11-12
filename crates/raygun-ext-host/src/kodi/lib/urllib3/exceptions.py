"""`urllib3.exceptions`."""


class HTTPError(Exception):
    pass


class HTTPWarning(Warning):
    pass


class PoolError(HTTPError):
    def __init__(self, pool=None, message=''):
        self.pool = pool
        super().__init__(message)


class RequestError(PoolError):
    def __init__(self, pool=None, url=None, message=''):
        self.url = url
        super().__init__(pool, message)


class MaxRetryError(RequestError):
    def __init__(self, pool=None, url=None, reason=None):
        self.reason = reason
        super().__init__(pool, url, 'Max retries exceeded with url: %s (Caused by %r)' % (url, reason))


class TimeoutError(HTTPError):
    pass


class ReadTimeoutError(TimeoutError, RequestError):
    pass


class ConnectTimeoutError(TimeoutError):
    pass


class NewConnectionError(ConnectTimeoutError, PoolError):
    pass


class SSLError(HTTPError):
    pass


class ProxyError(HTTPError):
    pass


class DecodeError(HTTPError):
    pass


class ProtocolError(HTTPError):
    pass


ConnectionError = ProtocolError


class LocationParseError(ValueError, HTTPError):
    pass


class ResponseError(HTTPError):
    pass


class InsecureRequestWarning(HTTPWarning):
    pass


class SecurityWarning(HTTPWarning):
    pass


class SystemTimeWarning(SecurityWarning):
    pass


class InsecurePlatformWarning(SecurityWarning):
    pass


class SNIMissingWarning(HTTPWarning):
    pass


class DependencyWarning(HTTPWarning):
    pass
