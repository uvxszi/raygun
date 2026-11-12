"""`open()` for Kodi add-ons in Raygun: whole files read and written through `_raygun`, which keeps
each add-on to its own folders. A file being written is kept in memory and saved when it's
flushed or closed."""

import io

import _raygun

_original_open = io.open


class _Reading(io.BytesIO):
    def __init__(self, path, data, mode):
        super().__init__(data)
        self._raygun_path = path
        self.name = path
        self.mode = mode


class _Writing(io.BytesIO):
    def __init__(self, path, data, mode, append):
        super().__init__(data)
        self._raygun_path = path
        self.name = path
        self.mode = mode
        self._saved = True
        if append:
            self.seek(0, io.SEEK_END)

    def write(self, data):
        self._saved = False
        return super().write(data)

    def writelines(self, lines):
        for line in lines:
            self.write(line)

    def truncate(self, size=None):
        self._saved = False
        return super().truncate(size)

    def flush(self):
        if not self.closed and not self._saved:
            _raygun.fs_write(self._raygun_path, self.getvalue(), False)
            self._saved = True

    def close(self):
        if not self.closed:
            self.flush()
        super().close()

    def __del__(self):
        try:
            self.close()
        except Exception:
            pass


def _binary(path, mode):
    kind = mode.replace('b', '').replace('t', '')
    plus = '+' in kind
    kind = kind.replace('+', '')
    if kind == 'r':
        data = _raygun.fs_read(path)
        if plus:
            return _Writing(path, data, mode, False)
        return _Reading(path, data, mode)
    if kind == 'w':
        handle = _Writing(path, b'', mode, False)
        handle._saved = False
        handle.flush()
        return handle
    if kind == 'x':
        try:
            _raygun.fs_stat(path)
        except FileNotFoundError:
            handle = _Writing(path, b'', mode, False)
            handle._saved = False
            handle.flush()
            return handle
        raise FileExistsError(17, 'File exists', path)
    if kind == 'a':
        try:
            data = _raygun.fs_read(path)
        except FileNotFoundError:
            data = b''
        handle = _Writing(path, data, mode, True)
        if not data:
            handle._saved = False
            handle.flush()
        return handle
    raise ValueError('invalid mode: %r' % mode)


def open(file, mode='r', buffering=-1, encoding=None, errors=None, newline=None, closefd=True,
         opener=None):
    if isinstance(file, int):
        import posix
        raw = posix._fd_file(file)
    else:
        import posix
        path = posix._text(file)
        raw = _binary(path, mode)
    if 'b' in mode:
        return raw
    text = io.TextIOWrapper(raw, encoding=encoding or 'utf-8', errors=errors, newline=newline)
    try:
        text.mode = mode
    except Exception:
        pass
    return text


def open_code(path):
    return open(path, 'rb')
