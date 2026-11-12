"""`requests.adapters`: connection pools and retries are Raygun's own; an adapter mounted on a
session changes nothing."""

DEFAULT_POOLBLOCK = False
DEFAULT_POOLSIZE = 10
DEFAULT_RETRIES = 0
DEFAULT_POOL_TIMEOUT = None


class BaseAdapter:
    def __init__(self, *args, **kwargs):
        pass

    def close(self):
        pass


class HTTPAdapter(BaseAdapter):
    def __init__(self, pool_connections=10, pool_maxsize=10, max_retries=0, pool_block=False,
                 **kwargs):
        self.max_retries = max_retries
        self.config = {}
        self.proxy_manager = {}

    def init_poolmanager(self, *args, **kwargs):
        pass

    def proxy_manager_for(self, *args, **kwargs):
        return None

    def cert_verify(self, *args, **kwargs):
        pass
