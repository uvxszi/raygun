"""`ctypes` for Kodi add-ons in Raygun: its names, for libraries that import it to reach a
system they may not be on; loading native code isn't possible."""


class _CData:
    def __init__(self, value=0):
        self.value = value


c_bool = c_char = c_wchar = c_byte = c_ubyte = c_short = c_ushort = c_int = c_uint = _CData
c_long = c_ulong = c_longlong = c_ulonglong = c_size_t = c_ssize_t = c_float = c_double = _CData
c_longdouble = c_char_p = c_wchar_p = c_void_p = c_int8 = c_int16 = c_int32 = c_int64 = _CData
c_uint8 = c_uint16 = c_uint32 = c_uint64 = _CData


class Structure:
    _fields_ = []


class Union(Structure):
    pass


class _Refused:
    def __init__(self, *args, **kwargs):
        raise OSError('Kodi add-ons in Raygun can\'t load native libraries')

    def __getattr__(self, name):
        raise OSError('Kodi add-ons in Raygun can\'t load native libraries')


CDLL = PyDLL = WinDLL = OleDLL = _Refused


class _Loader:
    def __getattr__(self, name):
        raise OSError('Kodi add-ons in Raygun can\'t load native libraries')

    def LoadLibrary(self, name):
        raise OSError('Kodi add-ons in Raygun can\'t load native libraries')


cdll = pydll = windll = oledll = _Loader()


def POINTER(kind):
    return _CData


def pointer(value):
    return value


def byref(value, offset=0):
    return value


def sizeof(value):
    return 8


def addressof(value):
    return 0


def cast(value, kind):
    return value


def create_string_buffer(init, size=None):
    return _CData(init)


create_unicode_buffer = create_string_buffer


def CFUNCTYPE(restype, *argtypes, **kwargs):
    return lambda function: function


WINFUNCTYPE = PYFUNCTYPE = CFUNCTYPE


def get_errno():
    return 0


def set_errno(value):
    return 0
