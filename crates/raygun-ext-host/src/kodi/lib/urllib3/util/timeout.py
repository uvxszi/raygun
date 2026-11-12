"""`urllib3.util.timeout`."""


class Timeout:
    DEFAULT_TIMEOUT = None

    def __init__(self, total=None, connect=None, read=None):
        self.total = total
        self._connect = connect
        self._read = read

    @property
    def connect_timeout(self):
        return self._connect or self.total

    @property
    def read_timeout(self):
        return self._read or self.total

    @classmethod
    def from_float(cls, timeout):
        return cls(read=timeout, connect=timeout)
