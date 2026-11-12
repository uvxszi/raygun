"""`platform` for Kodi add-ons in Raygun: a Kodi on Linux (Android's kernel) or Windows."""

import collections
import sys

import _raygun

uname_result = collections.namedtuple('uname_result',
                                      'system node release version machine processor')


def _desktop_windows():
    return _raygun.platform() == 'windows'


def system():
    return 'Windows' if _desktop_windows() else 'Linux'


def node():
    return 'raygun'


def release():
    return '10' if _desktop_windows() else '5.10'


def version():
    return '10.0.19045' if _desktop_windows() else '#1 SMP PREEMPT'


def machine():
    return _raygun.machine()


def processor():
    return machine()


def uname():
    return uname_result(system(), node(), release(), version(), machine(), processor())


def architecture(executable=sys.executable, bits='', linkage=''):
    return ('64bit', 'ELF' if not _desktop_windows() else 'WindowsPE')


def python_version():
    return '%d.%d.%d' % sys.version_info[:3]


def python_version_tuple():
    return tuple(str(part) for part in sys.version_info[:3])


def python_implementation():
    return 'CPython'


def python_build():
    return ('main', '')


def python_compiler():
    return 'Raygun'


def platform(aliased=False, terse=False):
    return '%s-%s-%s' % (system(), release(), machine())


def libc_ver(*args, **kwargs):
    return ('glibc', '2.31')


def mac_ver(*args, **kwargs):
    return ('', ('', '', ''), '')


def win32_ver(*args, **kwargs):
    return ('10', '10.0.19045', '', 'Multiprocessor Free') if _desktop_windows() else ('', '', '', '')


def win32_edition():
    return 'Core' if _desktop_windows() else ''


def freedesktop_os_release():
    raise OSError('no os-release')


def android_ver(*args, **kwargs):
    release = collections.namedtuple('AndroidVer', 'release api_level manufacturer model device is_emulator')
    return release('14', 34, 'Raygun', 'Raygun', 'raygun', False)
