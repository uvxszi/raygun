"""`StorageServer` for Kodi add-ons in Raygun (in place of `script.common.plugin.cache`, whose
service Kodi runs beside the add-on): the same cache, kept in the add-on's own folder."""

import hashlib
import pickle
import time

_FILE = '/kodi/home/userdata/addon_data/script.common.plugin.cache/cache.pickle'


def _load():
    try:
        with open(_FILE, 'rb') as handle:
            return pickle.load(handle)
    except Exception:
        return {}


def _save(store):
    import os
    try:
        os.makedirs(os.path.dirname(_FILE), exist_ok=True)
        with open(_FILE, 'wb') as handle:
            pickle.dump(store, handle)
    except Exception:
        pass


class StorageServer:
    def __init__(self, table=None, timeout=24, instance=False):
        self.table = table or 'cache'
        self.timeout = float(timeout) * 3600

    def _key(self, name):
        return '%s:%s' % (self.table, name)

    def cacheFunction(self, funct=False, *args):
        name = getattr(funct, '__name__', repr(funct))
        digest = hashlib.md5(repr((name, args)).encode('utf-8')).hexdigest()
        store = _load()
        key = self._key('function:' + digest)
        found = store.get(key)
        if found and time.time() - found[0] < self.timeout:
            return found[1]
        result = funct(*args)
        if result:
            store[key] = (time.time(), result)
            _save(store)
        return result

    def cacheDelete(self, name):
        store = _load()
        for key in [key for key in store if key.startswith(self._key(''))]:
            store.pop(key, None)
        _save(store)

    def cacheClean(self, empty=False):
        _save({})
        return True

    def lock(self, name):
        return True

    def unlock(self, name):
        return True

    def set(self, name, data):
        store = _load()
        store[self._key(name)] = (time.time(), data)
        _save(store)
        return ''

    def get(self, name):
        found = _load().get(self._key(name))
        return found[1] if found else ''

    def delete(self, name):
        store = _load()
        store.pop(self._key(name), None)
        _save(store)

    def setMulti(self, name, data):
        store = _load()
        for key, value in data.items():
            store[self._key('%s:%s' % (name, key))] = (time.time(), value)
        _save(store)
        return ''

    def getMulti(self, name, items):
        store = _load()
        found = []
        for item in items:
            value = store.get(self._key('%s:%s' % (name, item)))
            found.append(value[1] if value else '')
        return found
