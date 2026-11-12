"""`requests.auth`."""

import base64


def _basic_auth_str(username, password):
    if isinstance(username, str):
        username = username.encode('latin1')
    if isinstance(password, str):
        password = password.encode('latin1')
    return 'Basic ' + base64.b64encode(b':'.join((username, password))).strip().decode()


class AuthBase:
    def __call__(self, r):
        raise NotImplementedError('Auth hooks must be callable.')


class HTTPBasicAuth(AuthBase):
    def __init__(self, username, password):
        self.username = username
        self.password = password

    def __call__(self, r):
        r.headers['Authorization'] = _basic_auth_str(self.username, self.password)
        return r


class HTTPProxyAuth(HTTPBasicAuth):
    pass


class HTTPDigestAuth(HTTPBasicAuth):
    pass
