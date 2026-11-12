"""`distutils.util` (from CPython 3.11, PSF licence): what add-ons use of it."""


def strtobool(val):
    val = val.lower()
    if val in ('y', 'yes', 't', 'true', 'on', '1'):
        return 1
    if val in ('n', 'no', 'f', 'false', 'off', '0'):
        return 0
    raise ValueError('invalid truth value %r' % (val,))
