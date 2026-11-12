"""Starts the interpreter for one Kodi add-on: imports come from Raygun (the add-on, its
dependencies, Raygun's `xbmc*` modules and the standard library), files and the network go
through `_raygun`, and `run` runs the plugin once for a `plugin://` address."""

import sys

import _frozen_importlib as _bootstrap
import _raygun

_codes = {}


def _code(origin):
    code = _codes.get(origin)
    if code is None:
        source = _raygun.source(origin)
        code = compile(source, origin, 'exec', dont_inherit=True)
        _codes[origin] = code
    return code


class _Loader:
    @staticmethod
    def create_module(spec):
        return None

    @staticmethod
    def exec_module(module):
        origin = module.__spec__.origin
        module.__file__ = origin
        exec(_code(origin), module.__dict__)

    @staticmethod
    def get_source(fullname):
        spec = sys.modules[fullname].__spec__ if fullname in sys.modules else None
        if spec is None or not spec.origin:
            return None
        return _raygun.source(spec.origin)

    @staticmethod
    def is_package(fullname):
        found = _raygun.find_module(fullname, None)
        return bool(found and found[0] == 'package')

    @staticmethod
    def get_data(path):
        return _raygun.fs_read(path)

    @staticmethod
    def get_filename(fullname):
        module = sys.modules.get(fullname)
        return getattr(module, '__file__', None)


class _NamespaceLoader:
    """A folder without an `__init__.py` (a namespace package): nothing to run."""

    @staticmethod
    def create_module(spec):
        return None

    @staticmethod
    def exec_module(module):
        pass

    @staticmethod
    def is_package(fullname):
        return True


class _Finder:
    @classmethod
    def find_spec(cls, fullname, path=None, target=None):
        # Folders the add-on put on sys.path itself come first, as in Python.
        extra = [entry for entry in sys.path if isinstance(entry, str) and entry.startswith('/')]
        found = _raygun.find_module(fullname, list(path) if path is not None else None, extra)
        if found is None:
            return None
        kind = found[0]
        if kind == 'namespace':
            spec = _bootstrap.ModuleSpec(fullname, _NamespaceLoader, is_package=True)
            spec.submodule_search_locations = list(found[2])
            return spec
        spec = _bootstrap.ModuleSpec(fullname, _Loader, origin=found[1],
                                     is_package=kind == 'package')
        if kind == 'package':
            spec.submodule_search_locations = list(found[2])
        return spec

    @classmethod
    def invalidate_caches(cls):
        pass


class _Log:
    """`sys.stdout` and `sys.stderr`: lines go to Raygun's log of the add-on."""

    encoding = 'utf-8'
    errors = 'replace'

    def __init__(self, level):
        self._level = level
        self._pending = ''

    def write(self, text):
        if isinstance(text, bytes):
            text = text.decode('utf-8', 'replace')
        self._pending += str(text)
        while '\n' in self._pending:
            line, self._pending = self._pending.split('\n', 1)
            if line.strip():
                _raygun.log(self._level, line)
        return len(text)

    def flush(self):
        if self._pending.strip():
            _raygun.log(self._level, self._pending)
        self._pending = ''

    def isatty(self):
        return False

    def fileno(self):
        raise OSError('no file descriptor')

    def writable(self):
        return True

    def readable(self):
        return False


def _install():
    sys.meta_path.append(_Finder)
    sys.path_hooks.clear()
    sys.path[:] = []
    sys.dont_write_bytecode = True
    sys.stdout = _Log(1)
    sys.stderr = _Log(3)
    sys.__stdout__ = sys.stdout
    sys.__stderr__ = sys.stderr
    sys.stdin = None
    sys.executable = ''
    sys.platform = 'linux'
    sys.abiflags = ''

    import builtins
    import io
    import time

    import _raygun_files
    builtins.open = _raygun_files.open
    io.open = _raygun_files.open
    io.open_code = _raygun_files.open_code
    time.sleep = _raygun.sleep

    import encodings  # noqa: F401  (registers the codecs' search function)
    import posix
    posix.chdir('/kodi/home')

    sys.meta_path.insert(0, _AfterImport)


def _patch_http_client(module):
    import _raygun_http
    _raygun_http.patch_http_client(module)


def _patch_html_parser(module):
    # `HTMLParser().unescape`, gone from Python 3.9, which older libraries still call.
    import html
    if not hasattr(module.HTMLParser, 'unescape'):
        module.HTMLParser.unescape = lambda self, text: html.unescape(text)


_PATCHES = {
    'http.client': _patch_http_client,
    'html.parser': _patch_html_parser,
}


class _AfterImport:
    """Patches modules that need Raygun's parts as they're first imported: `http.client`'s
    connections go through Raygun's HTTP."""

    _busy = set()

    @classmethod
    def find_spec(cls, fullname, path=None, target=None):
        if fullname not in _PATCHES or fullname in cls._busy:
            return None
        cls._busy.add(fullname)
        try:
            spec = _bootstrap._find_spec(fullname, path, target)
        finally:
            cls._busy.discard(fullname)
        if spec is None:
            return None
        original = spec.loader

        class _Patched:
            @staticmethod
            def create_module(spec):
                return None

            @staticmethod
            def exec_module(module):
                original.exec_module(module)
                _PATCHES[fullname](module)

        spec.loader = _Patched
        return spec


def _purge():
    """Forgets the modules of the add-on and its dependencies, so each run starts afresh as in
    Kodi; Raygun's own modules and the standard library stay."""
    for name, module in list(sys.modules.items()):
        location = getattr(module, '__file__', None) or ''
        if not location:
            paths = getattr(module, '__path__', None)
            try:
                location = next(iter(paths)) if paths else ''
            except TypeError:
                location = ''
        if isinstance(location, str) and location.startswith('/kodi/home/addons/'):
            del sys.modules[name]
    sys.modules.pop('__main__', None)


def run(request_json):
    import json
    import traceback
    import types

    import _raygun_kodi
    import posix
    import xbmc
    request = json.loads(request_json)
    current = _raygun_kodi.start(request)
    _purge()
    sys.path[:] = []
    xbmc.PlayList._lists.clear()
    url = current.url
    base, sep, query = url.partition('?')
    sys.argv = [base, str(current.handle), ('?' + query) if sep else '', 'resume:false']
    entry = request['entry']
    # Files a plugin writes without a folder land in its temporary one (Kodi's own folder,
    # where they'd go in Kodi, isn't the plugin's to write in).
    try:
        posix.chdir('/kodi/temp')
    except OSError:
        pass
    main = types.ModuleType('__main__')
    main.__file__ = entry
    main.__loader__ = _Loader
    main.__builtins__ = __builtins__
    # Kodi's own runs start with these in the script's namespace.
    main.sys = sys
    main.xbmc = xbmc
    sys.modules['__main__'] = main
    error = None
    try:
        exec(_code(entry), main.__dict__)
    except SystemExit:
        pass
    except BaseException as raised:
        error = {
            'type': type(raised).__name__,
            'message': str(raised)[:2000],
            'traceback': ''.join(traceback.format_exception(raised))[-6000:],
        }
    try:
        sys.stdout.flush()
        sys.stderr.flush()
    except Exception:
        pass
    result = {
        'items': current.items,
        'ended': current.ended,
        'succeeded': current.succeeded,
        'content': current.content,
        'category': current.category,
        'resolved': current.resolved,
        'redirect': current.redirect,
        'played': current.played,
        'selects': current.selects,
        'messages': current.messages,
        'settings': current.settings_changed,
        'drm': current.drm,
        'error': error,
    }
    try:
        return json.dumps(result, default=str)
    except Exception as failed:
        return json.dumps({'items': [], 'error': {'type': 'TypeError', 'message': str(failed),
                                                  'traceback': ''}})


_install()
