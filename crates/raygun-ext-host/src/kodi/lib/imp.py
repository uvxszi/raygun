"""`imp`, gone from Python 3.12, for older add-ons: the parts they use, over `importlib`."""

import importlib
import sys
import types

PY_SOURCE = 1
PKG_DIRECTORY = 5
C_BUILTIN = 6


def reload(module):
    return importlib.reload(module)


def new_module(name):
    return types.ModuleType(name)


def get_magic():
    return importlib.util.MAGIC_NUMBER


def acquire_lock():
    pass


def release_lock():
    pass


def lock_held():
    return False


def load_source(name, pathname, file=None):
    source = open(pathname, encoding='utf-8').read()
    module = types.ModuleType(name)
    module.__file__ = pathname
    sys.modules[name] = module
    exec(compile(source, pathname, 'exec'), module.__dict__)
    return module


def find_module(name, path=None):
    spec = importlib.machinery.PathFinder.find_spec(name, path) if path else None
    if spec is None:
        spec = importlib.util.find_spec(name)
    if spec is None:
        raise ImportError('No module named %r' % name)
    return (None, spec.origin, ('', '', PY_SOURCE))


def load_module(name, file, pathname, description):
    return importlib.import_module(name)
