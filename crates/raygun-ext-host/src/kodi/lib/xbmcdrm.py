"""Kodi's `xbmcdrm` module: Raygun has no DRM."""


class CryptoSession:
    def __init__(self, UUID, cipherAlgorithm, macAlgorithm):
        import _raygun_kodi
        _raygun_kodi.note_drm(UUID)
        raise RuntimeError('DRM isn\'t available in Raygun')
