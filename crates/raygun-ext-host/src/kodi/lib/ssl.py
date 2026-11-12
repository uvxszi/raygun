"""`ssl` for Kodi add-ons in Raygun: Raygun's client makes every TLS connection and always checks
certificates, so contexts are kept only for the settings add-ons pass around."""

import enum as _enum

OPENSSL_VERSION = 'Raygun (rustls)'
OPENSSL_VERSION_INFO = (3, 0, 0, 0, 0)
OPENSSL_VERSION_NUMBER = 0x30000000
HAS_SNI = True
HAS_ALPN = True
HAS_ECDH = True
HAS_NPN = False
HAS_TLSv1_3 = True
CERT_NONE = 0
CERT_OPTIONAL = 1
CERT_REQUIRED = 2
PROTOCOL_SSLv23 = 2
PROTOCOL_TLS = 2
PROTOCOL_TLS_CLIENT = 16
PROTOCOL_TLS_SERVER = 17
PROTOCOL_TLSv1 = 3
PROTOCOL_TLSv1_1 = 4
PROTOCOL_TLSv1_2 = 5
OP_ALL = 0x80000054
OP_NO_SSLv2 = 0
OP_NO_SSLv3 = 0x02000000
OP_NO_TLSv1 = 0x04000000
OP_NO_TLSv1_1 = 0x10000000
OP_NO_TLSv1_2 = 0x08000000
OP_NO_TLSv1_3 = 0x20000000
OP_NO_COMPRESSION = 0x20000
OP_NO_TICKET = 0x4000
OP_CIPHER_SERVER_PREFERENCE = 0x400000
VERIFY_DEFAULT = 0
VERIFY_X509_STRICT = 0x20
VERIFY_X509_PARTIAL_CHAIN = 0x80000
CHANNEL_BINDING_TYPES = ['tls-unique']


class TLSVersion(_enum.IntEnum):
    MINIMUM_SUPPORTED = -2
    SSLv3 = 768
    TLSv1 = 769
    TLSv1_1 = 770
    TLSv1_2 = 771
    TLSv1_3 = 772
    MAXIMUM_SUPPORTED = -1


class Purpose(_enum.Enum):
    SERVER_AUTH = '1.3.6.1.5.5.7.3.1'
    CLIENT_AUTH = '1.3.6.1.5.5.7.3.2'


class SSLError(OSError):
    pass


class SSLZeroReturnError(SSLError):
    pass


class SSLWantReadError(SSLError):
    pass


class SSLWantWriteError(SSLError):
    pass


class SSLSyscallError(SSLError):
    pass


class SSLEOFError(SSLError):
    pass


class SSLCertVerificationError(SSLError, ValueError):
    pass


CertificateError = SSLCertVerificationError


class SSLContext:
    def __init__(self, protocol=PROTOCOL_TLS, *args, **kwargs):
        self.protocol = protocol
        self.check_hostname = protocol == PROTOCOL_TLS_CLIENT
        self.verify_mode = CERT_REQUIRED if self.check_hostname else CERT_NONE
        self.options = OP_ALL
        self.minimum_version = TLSVersion.MINIMUM_SUPPORTED
        self.maximum_version = TLSVersion.MAXIMUM_SUPPORTED
        self.verify_flags = VERIFY_DEFAULT
        self.post_handshake_auth = False
        self.keylog_filename = None
        self.sni_callback = None
        self.hostname_checks_common_name = False

    def load_verify_locations(self, cafile=None, capath=None, cadata=None):
        pass

    def load_cert_chain(self, certfile, keyfile=None, password=None):
        pass

    def load_default_certs(self, purpose=Purpose.SERVER_AUTH):
        pass

    def set_default_verify_paths(self):
        pass

    def set_ciphers(self, ciphers):
        pass

    def get_ciphers(self):
        return []

    def set_alpn_protocols(self, protocols):
        pass

    def set_npn_protocols(self, protocols):
        pass

    def set_ecdh_curve(self, name):
        pass

    def cert_store_stats(self):
        return {'x509': 0, 'crl': 0, 'x509_ca': 0}

    def get_ca_certs(self, binary_form=False):
        return []

    def wrap_socket(self, sock, *args, **kwargs):
        raise SSLError('Kodi add-ons in Raygun reach the network through urllib, http.client '
                       'or requests')

    def wrap_bio(self, *args, **kwargs):
        raise SSLError('not available')


class SSLSocket:
    pass


class SSLObject:
    pass


class MemoryBIO:
    pass


def create_default_context(purpose=Purpose.SERVER_AUTH, *, cafile=None, capath=None,
                           cadata=None):
    return SSLContext(PROTOCOL_TLS_CLIENT)


def _create_unverified_context(*args, **kwargs):
    context = SSLContext(PROTOCOL_TLS)
    context.check_hostname = False
    context.verify_mode = CERT_NONE
    return context


_create_default_https_context = create_default_context
_create_stdlib_context = _create_unverified_context


def wrap_socket(sock, *args, **kwargs):
    return SSLContext().wrap_socket(sock)


def match_hostname(cert, hostname):
    pass


def get_default_verify_paths():
    import collections
    paths = collections.namedtuple('DefaultVerifyPaths', 'cafile capath openssl_cafile_env '
                                   'openssl_cafile openssl_capath_env openssl_capath')
    return paths(None, None, 'SSL_CERT_FILE', '', 'SSL_CERT_DIR', '')


def get_server_certificate(addr, *args, **kwargs):
    raise SSLError('not available')


def DER_cert_to_PEM_cert(der):
    import base64
    text = base64.encodebytes(der).decode('ascii')
    return '-----BEGIN CERTIFICATE-----\n' + text + '-----END CERTIFICATE-----\n'


def PEM_cert_to_DER_cert(pem):
    import base64
    body = pem.split('-----')[2]
    return base64.decodebytes(body.encode('ascii'))


def RAND_bytes(size):
    import os
    return os.urandom(size)


def RAND_status():
    return True
