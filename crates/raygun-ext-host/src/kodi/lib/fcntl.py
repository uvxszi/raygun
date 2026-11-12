"""`fcntl` for Kodi add-ons in Raygun: there are no descriptors to lock; locks always succeed."""

LOCK_SH = 1
LOCK_EX = 2
LOCK_NB = 4
LOCK_UN = 8
F_GETFD = 1
F_SETFD = 2
F_GETFL = 3
F_SETFL = 4
FD_CLOEXEC = 1


def flock(fd, operation):
    pass


def lockf(fd, cmd, len=0, start=0, whence=0):
    pass


def fcntl(fd, cmd, arg=0):
    return 0


def ioctl(fd, request, arg=0, mutate_flag=True):
    return 0
