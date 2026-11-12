"""`urllib3.util.retry`: Raygun retries on its own; a `Retry` changes nothing."""


class Retry:
    DEFAULT_ALLOWED_METHODS = frozenset(['HEAD', 'GET', 'PUT', 'DELETE', 'OPTIONS', 'TRACE'])
    DEFAULT_METHOD_WHITELIST = DEFAULT_ALLOWED_METHODS
    RETRY_AFTER_STATUS_CODES = frozenset([413, 429, 503])
    BACKOFF_MAX = 120

    def __init__(self, total=10, connect=None, read=None, redirect=None, status=None,
                 other=None, allowed_methods=None, status_forcelist=None, backoff_factor=0,
                 raise_on_redirect=True, raise_on_status=True, history=None,
                 respect_retry_after_header=True, remove_headers_on_redirect=None,
                 method_whitelist=None, **kwargs):
        self.total = total
        self.connect = connect
        self.read = read
        self.redirect = redirect
        self.status = status
        self.backoff_factor = backoff_factor
        self.status_forcelist = status_forcelist or set()

    @classmethod
    def from_int(cls, retries, redirect=True, default=None):
        return retries if isinstance(retries, Retry) else cls(retries)

    def new(self, **kwargs):
        return self

    def is_exhausted(self):
        return False


Retry.DEFAULT = Retry(3)
