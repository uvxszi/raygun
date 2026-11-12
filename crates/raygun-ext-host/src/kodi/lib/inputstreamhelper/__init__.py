"""`inputstreamhelper` for Kodi add-ons in Raygun (in place of `script.module.inputstreamhelper`):
Raygun's player opens HLS and DASH itself; streams that need DRM it can't play."""

import xbmc


class Helper:
    def __init__(self, protocol, drm=None):
        self.protocol = protocol
        self.drm = drm
        self.inputstream_addon = 'inputstream.adaptive'

    def check_inputstream(self):
        if self.drm:
            xbmc.log('[inputstreamhelper] DRM (%s) isn\'t available in Raygun' % self.drm,
                     xbmc.LOGWARNING)
            import _raygun_kodi
            _raygun_kodi.note_drm(self.drm)
            return False
        return True

    def install_inputstream(self):
        return True

    def install_widevine(self):
        return False

    def remove_widevine(self):
        return False

    def info_dialog(self):
        pass

    def _inputstream_version(self):
        return '21.4.4'


def has_inputstream(addon='inputstream.adaptive'):
    return True
