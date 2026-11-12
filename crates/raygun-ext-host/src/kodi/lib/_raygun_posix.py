"""The `posix` module as Kodi add-ons see it in Raygun.

The interpreter has no operating system of its own: paths are Kodi's (`/kodi/home/addons/...`,
`/kodi/home/userdata/...`, `/kodi/temp/...`) and every file call goes through `_raygun`, which
keeps an add-on to its own folders. Processes, sockets, signals and the like don't exist here.
"""

import _raygun

name = 'posix'

F_OK = 0
X_OK = 1
W_OK = 2
R_OK = 4
O_RDONLY = 0
O_WRONLY = 1
O_RDWR = 2
O_CREAT = 0o100
O_EXCL = 0o200
O_NOCTTY = 0o400
O_TRUNC = 0o1000
O_APPEND = 0o2000
O_NONBLOCK = 0o4000
O_CLOEXEC = 0o2000000
O_NOFOLLOW = 0o400000
O_DIRECTORY = 0o200000
O_BINARY = 0
SEEK_SET = 0
SEEK_CUR = 1
SEEK_END = 2
EX_OK = 0
WNOHANG = 1

environ = {
    b'HOME': b'/kodi/home',
    b'TMPDIR': b'/kodi/temp',
    b'LANG': b'en_US.UTF-8',
    b'PATH': b'',
}

_have_functions = []
_cwd = '/kodi/home'


class stat_result(tuple):
    n_sequence_fields = 10
    n_fields = 10
    n_unnamed_fields = 0

    def __new__(cls, values, extra=None):
        values = tuple(values)[:10]
        return tuple.__new__(cls, values)

    st_mode = property(lambda self: self[0])
    st_ino = property(lambda self: self[1])
    st_dev = property(lambda self: self[2])
    st_nlink = property(lambda self: self[3])
    st_uid = property(lambda self: self[4])
    st_gid = property(lambda self: self[5])
    st_size = property(lambda self: self[6])
    st_atime = property(lambda self: float(self[7]))
    st_mtime = property(lambda self: float(self[8]))
    st_ctime = property(lambda self: float(self[9]))
    st_atime_ns = property(lambda self: int(self[7] * 1e9))
    st_mtime_ns = property(lambda self: int(self[8] * 1e9))
    st_ctime_ns = property(lambda self: int(self[9] * 1e9))
    st_blksize = property(lambda self: 4096)
    st_blocks = property(lambda self: (self[6] + 511) // 512)
    st_flags = property(lambda self: 0)
    st_rdev = property(lambda self: 0)

    def __repr__(self):
        return 'os.stat_result(st_mode=%d, st_size=%d, st_mtime=%r)' % (self[0], self[6], self[8])


class statvfs_result(tuple):
    pass


class uname_result(tuple):
    sysname = property(lambda self: self[0])
    nodename = property(lambda self: self[1])
    release = property(lambda self: self[2])
    version = property(lambda self: self[3])
    machine = property(lambda self: self[4])


class times_result(tuple):
    user = property(lambda self: self[0])
    system = property(lambda self: self[1])
    children_user = property(lambda self: 0.0)
    children_system = property(lambda self: 0.0)
    elapsed = property(lambda self: self[4])


class terminal_size(tuple):
    columns = property(lambda self: self[0])
    lines = property(lambda self: self[1])


def fspath(path):
    if isinstance(path, (str, bytes)):
        return path
    kind = type(path)
    try:
        value = kind.__fspath__(path)
    except AttributeError:
        raise TypeError('expected str, bytes or os.PathLike object, not ' + kind.__name__)
    if isinstance(value, (str, bytes)):
        return value
    raise TypeError('expected %s.__fspath__() to return str or bytes' % kind.__name__)


def _text(path):
    path = fspath(path)
    if isinstance(path, bytes):
        path = path.decode('utf-8', 'surrogateescape')
    if isinstance(path, int):
        raise OSError(9, 'Bad file descriptor')
    if not path.startswith('/'):
        path = _cwd.rstrip('/') + '/' + path
    return path


def _same_kind(original, value):
    if isinstance(fspath(original), bytes):
        return value.encode('utf-8', 'surrogateescape')
    return value


def stat(path, *, dir_fd=None, follow_symlinks=True):
    if isinstance(path, int):
        return fstat(path)
    is_dir, size, mtime = _raygun.fs_stat(_text(path))
    mode = (0o040755 if is_dir else 0o100644)
    return stat_result((mode, 0, 0, 1, 0, 0, size, mtime, mtime, mtime))


lstat = stat


def access(path, mode, *, dir_fd=None, effective_ids=False, follow_symlinks=True):
    try:
        _raygun.fs_stat(_text(path))
    except OSError:
        return False
    if mode & W_OK:
        return _raygun.fs_writable(_text(path))
    return True


def listdir(path='.'):
    if isinstance(path, int):
        raise OSError(9, 'Bad file descriptor')
    names = _raygun.fs_list(_text(path))
    return [_same_kind(path, name) for name in names]


class DirEntry:
    def __init__(self, folder, name, original):
        self._full = folder.rstrip('/') + '/' + name
        self.name = _same_kind(original, name)
        self.path = _same_kind(original, self._full)
        self._stat = None

    def _info(self):
        if self._stat is None:
            self._stat = stat(self._full)
        return self._stat

    def is_dir(self, *, follow_symlinks=True):
        try:
            return (self._info().st_mode & 0o170000) == 0o040000
        except OSError:
            return False

    def is_file(self, *, follow_symlinks=True):
        try:
            return (self._info().st_mode & 0o170000) == 0o100000
        except OSError:
            return False

    def is_symlink(self):
        return False

    def is_junction(self):
        return False

    def stat(self, *, follow_symlinks=True):
        return self._info()

    def inode(self):
        return 0

    def __fspath__(self):
        return self.path

    def __repr__(self):
        return '<DirEntry %r>' % (self.name,)


class _ScandirIterator:
    def __init__(self, entries):
        self._entries = iter(entries)

    def __iter__(self):
        return self

    def __next__(self):
        return next(self._entries)

    def close(self):
        self._entries = iter(())

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False


def scandir(path='.'):
    folder = _text(path)
    names = _raygun.fs_list(folder)
    return _ScandirIterator([DirEntry(folder, name, path) for name in names])


def mkdir(path, mode=0o777, *, dir_fd=None):
    _raygun.fs_mkdir(_text(path))


def rmdir(path, *, dir_fd=None):
    _raygun.fs_rmdir(_text(path))


def unlink(path, *, dir_fd=None):
    _raygun.fs_remove(_text(path))


remove = unlink


def rename(src, dst, *, src_dir_fd=None, dst_dir_fd=None):
    _raygun.fs_rename(_text(src), _text(dst))


replace = rename


def getcwd():
    return _cwd


def getcwdb():
    return _cwd.encode()


def chdir(path):
    global _cwd
    target = _text(path)
    is_dir, _, _ = _raygun.fs_stat(target)
    if not is_dir:
        raise NotADirectoryError(20, 'Not a directory', path)
    _cwd = target


def chmod(path, mode, *, dir_fd=None, follow_symlinks=True):
    _raygun.fs_stat(_text(path))


def utime(path, times=None, *, ns=None, dir_fd=None, follow_symlinks=True):
    _raygun.fs_stat(_text(path))


def truncate(path, length):
    data = _raygun.fs_read(_text(path))[:length]
    _raygun.fs_write(_text(path), data, False)


def readlink(path, *, dir_fd=None):
    raise OSError(22, 'Invalid argument', path)


def _refused(*args, **kwargs):
    raise PermissionError(1, 'Not available to Kodi add-ons in Raygun')


symlink = link = mkfifo = mknod = chown = lchown = chroot = _refused
system = execv = execve = execvp = spawnv = kill = killpg = pipe = _refused
waitpid = wait = setsid = setuid = setgid = nice = _refused


def urandom(size):
    return _raygun.urandom(size)


def getpid():
    return 4242


def getppid():
    return 1


def getuid():
    return 10042


geteuid = getgid = getegid = getuid


def getlogin():
    return 'kodi'


def getgroups():
    return []


def umask(mask):
    return 0o022


def cpu_count():
    return 2


def strerror(code):
    return 'error %d' % code


def device_encoding(fd):
    return None


def isatty(fd):
    return False


def get_terminal_size(fd=1):
    raise OSError(25, 'Not a terminal')


def get_blocking(fd):
    return True


def set_blocking(fd, blocking):
    pass


def get_inheritable(fd):
    return False


def set_inheritable(fd, inheritable):
    pass


def uname():
    return uname_result(('Linux', 'raygun', '5.10', '#1', 'aarch64'))


def times():
    import time
    return times_result((time.process_time(), 0.0, 0.0, 0.0, time.monotonic()))


def putenv(key, value):
    environ[key if isinstance(key, bytes) else key.encode()] = (
        value if isinstance(value, bytes) else value.encode())


def unsetenv(key):
    environ.pop(key if isinstance(key, bytes) else key.encode(), None)


def _exit(code):
    raise SystemExit(code)


def abort():
    raise SystemExit(134)


def fsync(fd):
    pass


def sync():
    pass


# File descriptors: files opened by `os.open` (tempfile does), kept as Python file objects.
_files = {}
_next_fd = [10]


def open(path, flags, mode=0o777, *, dir_fd=None):
    import _raygun_files
    target = _text(path)
    exists = True
    try:
        is_dir, _, _ = _raygun.fs_stat(target)
    except FileNotFoundError:
        exists = False
        is_dir = False
    if is_dir:
        raise IsADirectoryError(21, 'Is a directory', path)
    if exists and flags & O_CREAT and flags & O_EXCL:
        raise FileExistsError(17, 'File exists', path)
    if not exists and not flags & O_CREAT:
        raise FileNotFoundError(2, 'No such file or directory', path)
    writing = flags & (O_WRONLY | O_RDWR)
    if writing:
        if flags & O_APPEND:
            file_mode = 'ab+'
        elif flags & O_TRUNC or not exists:
            file_mode = 'wb+'
        else:
            file_mode = 'rb+'
    else:
        file_mode = 'rb'
    handle = _raygun_files.open(target, file_mode)
    fd = _next_fd[0]
    _next_fd[0] += 1
    _files[fd] = handle
    return fd


def _file(fd):
    try:
        return _files[fd]
    except KeyError:
        raise OSError(9, 'Bad file descriptor')


def close(fd):
    handle = _files.pop(fd, None)
    if handle is None:
        raise OSError(9, 'Bad file descriptor')
    handle.close()


def closerange(low, high):
    for fd in list(_files):
        if low <= fd < high:
            close(fd)


def read(fd, size):
    return _file(fd).read(size)


def write(fd, data):
    if fd in (1, 2):
        import sys
        stream = sys.stdout if fd == 1 else sys.stderr
        stream.write(bytes(data).decode('utf-8', 'replace'))
        return len(data)
    return _file(fd).write(bytes(data))


def lseek(fd, position, how):
    return _file(fd).seek(position, how)


def ftruncate(fd, length):
    _file(fd).truncate(length)


def fstat(fd):
    handle = _file(fd)
    if hasattr(handle, '_raygun_path'):
        try:
            return stat(handle._raygun_path)
        except OSError:
            pass
    size = len(handle.getbuffer()) if hasattr(handle, 'getbuffer') else 0
    return stat_result((0o100644, 0, 0, 1, 0, 0, size, 0, 0, 0))


def dup(fd):
    raise OSError(9, 'Bad file descriptor')


def _fd_file(fd):
    """The file object behind `fd`, for `open(fd)`."""
    return _file(fd)
