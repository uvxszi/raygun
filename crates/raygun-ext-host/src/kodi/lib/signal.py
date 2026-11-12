"""`signal` for Kodi add-ons in Raygun: handlers are kept, never called."""

import enum as _enum

SIG_DFL = 0
SIG_IGN = 1
NSIG = 65


class Signals(_enum.IntEnum):
    SIGHUP = 1
    SIGINT = 2
    SIGQUIT = 3
    SIGILL = 4
    SIGABRT = 6
    SIGKILL = 9
    SIGUSR1 = 10
    SIGSEGV = 11
    SIGUSR2 = 12
    SIGPIPE = 13
    SIGALRM = 14
    SIGTERM = 15
    SIGCHLD = 17


class Handlers(_enum.IntEnum):
    SIG_DFL = 0
    SIG_IGN = 1


globals().update(Signals.__members__)
_handlers = {}


def signal(signalnum, handler):
    previous = _handlers.get(int(signalnum), SIG_DFL)
    _handlers[int(signalnum)] = handler
    return previous


def getsignal(signalnum):
    return _handlers.get(int(signalnum), SIG_DFL)


def default_int_handler(*args):
    raise KeyboardInterrupt


def alarm(seconds):
    return 0


def pause():
    pass


def set_wakeup_fd(fd, *, warn_on_full_buffer=True):
    return -1


def siginterrupt(signalnum, flag):
    pass


def raise_signal(signalnum):
    pass


def strsignal(signalnum):
    return str(signalnum)


def valid_signals():
    return set(Signals)
