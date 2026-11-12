"""Kodi's `xbmcvfs` module, as Raygun runs plugins: Kodi's `special://` paths turned into the
add-on's own folders, and files in them."""

import os

import _raygun

_SPECIAL = {
    'home': '/kodi/home',
    'profile': '/kodi/home/userdata',
    'masterprofile': '/kodi/home/userdata',
    'userdata': '/kodi/home/userdata',
    'database': '/kodi/home/userdata/Database',
    'thumbnails': '/kodi/home/userdata/Thumbnails',
    'temp': '/kodi/temp',
    'logpath': '/kodi/temp',
    'xbmc': '/kodi/xbmc',
    'xbmcbin': '/kodi/xbmc',
    'xbmcbinaddons': '/kodi/xbmc/addons',
    'subtitles': '/kodi/temp/subtitles',
    'recordings': '/kodi/home/userdata/recordings',
    'screenshots': '/kodi/home/userdata/screenshots',
    'videoplaylists': '/kodi/home/userdata/playlists/video',
    'musicplaylists': '/kodi/home/userdata/playlists/music',
    'cdrips': '/kodi/home/userdata/cdrips',
    'skin': '/kodi/xbmc/addons/skin.estuary',
    'frameworks': '/kodi/xbmc',
}


def translatePath(path):
    if path is None:
        return ''
    text = path if isinstance(path, str) else os.fsdecode(path)
    if text.startswith('special://'):
        rest = text[10:]
        name, _, tail = rest.partition('/')
        base = _SPECIAL.get(name.lower())
        if base is None:
            return text
        return base + ('/' + tail if tail else '')
    if text.startswith('resource://'):
        return '/kodi/home/addons/' + text[11:]
    return text


def _path(path):
    return translatePath(path)


def exists(path):
    target = _path(path)
    if not target.startswith('/'):
        return False
    try:
        is_dir, _, _ = _raygun.fs_stat(target.rstrip('/') or '/')
    except OSError:
        return False
    if path.endswith('/') and not is_dir:
        return False
    return True


def mkdir(path):
    try:
        _raygun.fs_mkdir(_path(path).rstrip('/'))
        return True
    except FileExistsError:
        return True
    except OSError:
        return False


def mkdirs(path):
    try:
        os.makedirs(_path(path).rstrip('/'), exist_ok=True)
        return True
    except OSError:
        return False


def rmdir(path, force=False):
    target = _path(path).rstrip('/')
    try:
        if force:
            import shutil
            shutil.rmtree(target)
        else:
            _raygun.fs_rmdir(target)
        return True
    except OSError:
        return False


def delete(file):
    try:
        _raygun.fs_remove(_path(file))
        return True
    except OSError:
        return False


def rename(file, newFileName):
    try:
        _raygun.fs_rename(_path(file), _path(newFileName))
        return True
    except OSError:
        return False


def copy(strSource, strDestination):
    try:
        data = _raygun.fs_read(_path(strSource))
        _raygun.fs_write(_path(strDestination), data, False)
        return True
    except OSError:
        return False


def listdir(path):
    target = _path(path).rstrip('/')
    dirs, files = [], []
    try:
        names = _raygun.fs_list(target)
    except OSError:
        return dirs, files
    for name in names:
        try:
            is_dir, _, _ = _raygun.fs_stat(target + '/' + name)
        except OSError:
            continue
        (dirs if is_dir else files).append(name)
    return dirs, files


def makeLegalFilename(filename):
    import re
    return re.sub(r'[\\:*?"<>|]', '_', filename)


def validatePath(path):
    return path


class File:
    def __init__(self, filepath, mode=None):
        self._path = _path(filepath)
        self._mode = mode or 'r'
        if self._mode in ('w', 'wb'):
            self._data = bytearray()
            self._writing = True
        else:
            self._writing = False
            try:
                self._data = bytearray(_raygun.fs_read(self._path))
            except OSError:
                self._data = bytearray()
        self._position = 0

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    def read(self, numBytes=0):
        end = len(self._data) if not numBytes or numBytes < 0 else self._position + numBytes
        chunk = bytes(self._data[self._position:end])
        self._position += len(chunk)
        return chunk.decode('utf-8', 'replace')

    def readBytes(self, numBytes=0):
        end = len(self._data) if not numBytes or numBytes < 0 else self._position + numBytes
        chunk = bytes(self._data[self._position:end])
        self._position += len(chunk)
        return bytearray(chunk)

    def write(self, buffer):
        if isinstance(buffer, str):
            buffer = buffer.encode('utf-8')
        self._data[self._position:self._position + len(buffer)] = buffer
        self._position += len(buffer)
        self._writing = True
        return True

    def size(self):
        return len(self._data)

    def seek(self, seekBytes, iWhence=0):
        if iWhence == 1:
            self._position += seekBytes
        elif iWhence == 2:
            self._position = len(self._data) + seekBytes
        else:
            self._position = seekBytes
        return self._position

    def tell(self):
        return self._position

    def close(self):
        if self._writing:
            try:
                _raygun.fs_write(self._path, bytes(self._data), False)
            except OSError:
                pass
            self._writing = False


class Stat:
    def __init__(self, path):
        try:
            is_dir, size, mtime = _raygun.fs_stat(_path(path))
        except OSError:
            is_dir, size, mtime = False, 0, 0
        self._dir = is_dir
        self._size = size
        self._mtime = int(mtime)

    def st_mode(self):
        return 0o040755 if self._dir else 0o100644

    def st_ino(self):
        return 0

    def st_dev(self):
        return 0

    def st_nlink(self):
        return 1

    def st_uid(self):
        return 0

    def st_gid(self):
        return 0

    def st_size(self):
        return self._size

    def st_atime(self):
        return self._mtime

    def st_mtime(self):
        return self._mtime

    def st_ctime(self):
        return self._mtime

