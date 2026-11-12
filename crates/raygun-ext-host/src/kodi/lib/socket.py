"""`socket` for Kodi add-ons in Raygun: names and constants only. Add-ons reach the network
through `urllib`, `http.client` or `requests`, which Raygun answers; a socket of their own can't
connect."""

import errno as _errno

AF_UNSPEC = 0
AF_UNIX = 1
AF_INET = 2
AF_INET6 = 10
SOCK_STREAM = 1
SOCK_DGRAM = 2
SOCK_RAW = 3
SOCK_CLOEXEC = 0o2000000
SOCK_NONBLOCK = 0o4000
SOL_SOCKET = 1
SOL_TCP = 6
SO_REUSEADDR = 2
SO_KEEPALIVE = 9
SO_BROADCAST = 6
SO_RCVBUF = 8
SO_SNDBUF = 7
SO_ERROR = 4
IPPROTO_IP = 0
IPPROTO_TCP = 6
IPPROTO_UDP = 17
IPPROTO_IPV6 = 41
TCP_NODELAY = 1
TCP_KEEPIDLE = 4
TCP_KEEPINTVL = 5
TCP_KEEPCNT = 6
IP_MULTICAST_TTL = 33
IP_ADD_MEMBERSHIP = 35
INADDR_ANY = 0
SHUT_RD = 0
SHUT_WR = 1
SHUT_RDWR = 2
MSG_PEEK = 2
AI_PASSIVE = 1
AI_CANONNAME = 2
AI_NUMERICHOST = 4
AI_ADDRCONFIG = 32
NI_NUMERICHOST = 1
has_ipv6 = True
_GLOBAL_DEFAULT_TIMEOUT = object()
_default_timeout = None

error = OSError
timeout = TimeoutError


class herror(OSError):
    pass


class gaierror(OSError):
    pass


EAI_NONAME = -2


def _refused():
    return PermissionError(_errno.EPERM, 'Kodi add-ons in Raygun reach the network through '
                                         'urllib, http.client or requests')


class socket:
    def __init__(self, family=AF_INET, type=SOCK_STREAM, proto=0, fileno=None):
        self.family = family
        self.type = type
        self.proto = proto
        self._timeout = _default_timeout

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    def settimeout(self, value):
        self._timeout = value

    def gettimeout(self):
        return self._timeout

    def setblocking(self, flag):
        pass

    def setsockopt(self, *args):
        pass

    def getsockopt(self, *args):
        return 0

    def connect(self, address):
        raise _refused()

    connect_ex = connect

    def bind(self, address):
        raise _refused()

    def listen(self, *args):
        raise _refused()

    def accept(self):
        raise _refused()

    def send(self, data, flags=0):
        raise _refused()

    sendall = sendto = recv = recvfrom = recv_into = send

    def makefile(self, *args, **kwargs):
        raise _refused()

    def fileno(self):
        return -1

    def shutdown(self, how):
        pass

    def close(self):
        pass

    def detach(self):
        return -1

    def getpeername(self):
        raise OSError(_errno.ENOTCONN, 'not connected')

    def getsockname(self):
        return ('0.0.0.0', 0)


SocketType = socket


def create_connection(address, timeout=_GLOBAL_DEFAULT_TIMEOUT, source_address=None, *args,
                      **kwargs):
    raise _refused()


def create_server(*args, **kwargs):
    raise _refused()


def socketpair(*args, **kwargs):
    raise _refused()


def fromfd(*args, **kwargs):
    raise _refused()


def getaddrinfo(host, port, family=0, type=0, proto=0, flags=0):
    raise gaierror(EAI_NONAME, 'Kodi add-ons in Raygun resolve no names themselves')


def gethostbyname(hostname):
    raise gaierror(EAI_NONAME, 'Kodi add-ons in Raygun resolve no names themselves')


def gethostbyname_ex(hostname):
    raise gaierror(EAI_NONAME, 'Kodi add-ons in Raygun resolve no names themselves')


def gethostbyaddr(address):
    raise herror(1, 'Unknown host')


def getnameinfo(sockaddr, flags):
    return (sockaddr[0], str(sockaddr[1]))


def gethostname():
    return 'raygun'


def getfqdn(name=''):
    return name or 'raygun'


def getdefaulttimeout():
    return _default_timeout


def setdefaulttimeout(value):
    global _default_timeout
    _default_timeout = value


def inet_aton(text):
    parts = text.split('.')
    if len(parts) != 4:
        raise OSError('illegal IP address string passed to inet_aton')
    return bytes(int(part) for part in parts)


def inet_ntoa(packed):
    return '.'.join(str(byte) for byte in packed)


def inet_pton(family, text):
    import ipaddress
    if family == AF_INET:
        return ipaddress.IPv4Address(text).packed
    return ipaddress.IPv6Address(text).packed


def inet_ntop(family, packed):
    import ipaddress
    if family == AF_INET:
        return str(ipaddress.IPv4Address(packed))
    return str(ipaddress.IPv6Address(packed))


def htons(value):
    return ((value & 0xff) << 8) | ((value >> 8) & 0xff)


ntohs = htons


def htonl(value):
    return int.from_bytes(value.to_bytes(4, 'little'), 'big')


ntohl = htonl
