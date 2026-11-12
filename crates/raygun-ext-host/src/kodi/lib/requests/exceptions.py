"""`requests.exceptions`."""

import json


class RequestException(IOError):
    def __init__(self, *args, **kwargs):
        self.response = kwargs.pop('response', None)
        self.request = kwargs.pop('request', None)
        if self.response is not None and self.request is None:
            self.request = getattr(self.response, 'request', None)
        super().__init__(*args)


class InvalidJSONError(RequestException):
    pass


class JSONDecodeError(InvalidJSONError, json.JSONDecodeError):
    def __init__(self, *args, **kwargs):
        try:
            json.JSONDecodeError.__init__(self, *args)
        except TypeError:
            InvalidJSONError.__init__(self, *args)


class HTTPError(RequestException):
    pass


class ConnectionError(RequestException):
    pass


class ProxyError(ConnectionError):
    pass


class SSLError(ConnectionError):
    pass


class Timeout(RequestException):
    pass


class ConnectTimeout(ConnectionError, Timeout):
    pass


class ReadTimeout(Timeout):
    pass


class URLRequired(RequestException):
    pass


class TooManyRedirects(RequestException):
    pass


class MissingSchema(RequestException, ValueError):
    pass


class InvalidSchema(RequestException, ValueError):
    pass


class InvalidURL(RequestException, ValueError):
    pass


class InvalidHeader(RequestException, ValueError):
    pass


class ChunkedEncodingError(RequestException):
    pass


class ContentDecodingError(RequestException):
    pass


class StreamConsumedError(RequestException, TypeError):
    pass


class RetryError(RequestException):
    pass


class RequestsWarning(Warning):
    pass


class DependencyWarning(RequestsWarning):
    pass
