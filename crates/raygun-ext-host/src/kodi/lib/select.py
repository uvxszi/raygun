"""`select` for Kodi add-ons in Raygun: there are no sockets or pipes to wait on."""

error = OSError
POLLIN = 1
POLLPRI = 2
POLLOUT = 4
POLLERR = 8
POLLHUP = 16
POLLNVAL = 32


def select(rlist, wlist, xlist, timeout=None):
    raise OSError('Kodi add-ons in Raygun have no sockets to wait on')


class poll:
    def register(self, fd, eventmask=None):
        pass

    def modify(self, fd, eventmask):
        pass

    def unregister(self, fd):
        pass

    def poll(self, timeout=None):
        return []
