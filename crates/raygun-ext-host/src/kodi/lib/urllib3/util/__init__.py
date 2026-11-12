"""`urllib3.util`."""

from .retry import Retry  # noqa: F401
from .timeout import Timeout  # noqa: F401
from .url import Url, parse_url  # noqa: F401


def make_headers(keep_alive=None, accept_encoding=None, user_agent=None, basic_auth=None,
                 proxy_basic_auth=None, disable_cache=None):
    import base64
    headers = {}
    if accept_encoding:
        headers['accept-encoding'] = (accept_encoding if isinstance(accept_encoding, str)
                                      else 'gzip,deflate')
    if user_agent:
        headers['user-agent'] = user_agent
    if keep_alive:
        headers['connection'] = 'keep-alive'
    if basic_auth:
        headers['authorization'] = 'Basic ' + base64.b64encode(basic_auth.encode()).decode()
    if disable_cache:
        headers['cache-control'] = 'no-cache'
    return headers
