"""Kodi's `xbmcaddon` module, as Raygun runs plugins: an add-on's details, its settings (the
values set in Raygun, else its own defaults) and its localized strings."""

import _raygun_kodi


class Settings:
    def __init__(self, addon_id):
        self._id = addon_id

    def getBool(self, id):
        return _raygun_kodi.setting(self._id, id).lower() == 'true'

    def getInt(self, id):
        try:
            return int(float(_raygun_kodi.setting(self._id, id) or 0))
        except ValueError:
            return 0

    def getNumber(self, id):
        try:
            return float(_raygun_kodi.setting(self._id, id) or 0)
        except ValueError:
            return 0.0

    def getString(self, id):
        return _raygun_kodi.setting(self._id, id)

    def getBoolList(self, id):
        return [part.lower() == 'true' for part in self.getString(id).split('|') if part]

    def getIntList(self, id):
        return [int(part) for part in self.getString(id).split('|') if part.strip().isdigit()]

    def getNumberList(self, id):
        return [float(part) for part in self.getString(id).split('|') if part]

    def getStringList(self, id):
        return [part for part in self.getString(id).split('|') if part]

    def setBool(self, id, value):
        _raygun_kodi.set_setting(self._id, id, bool(value))

    def setInt(self, id, value):
        _raygun_kodi.set_setting(self._id, id, int(value))

    def setNumber(self, id, value):
        _raygun_kodi.set_setting(self._id, id, float(value))

    def setString(self, id, value):
        _raygun_kodi.set_setting(self._id, id, value)

    def setBoolList(self, id, values):
        _raygun_kodi.set_setting(self._id, id, '|'.join('true' if v else 'false' for v in values))

    def setIntList(self, id, values):
        _raygun_kodi.set_setting(self._id, id, '|'.join(str(int(v)) for v in values))

    def setNumberList(self, id, values):
        _raygun_kodi.set_setting(self._id, id, '|'.join(str(float(v)) for v in values))

    def setStringList(self, id, values):
        _raygun_kodi.set_setting(self._id, id, '|'.join(values))


class Addon:
    def __init__(self, id=None):
        if id == 'xbmc.addon' or id == 'xbmc.python':
            self._id = id
            self._info = {'id': id, 'name': 'Kodi', 'version': _raygun_kodi.KODI_VERSION,
                          'path': '/kodi/xbmc', 'settings': {}}
            return
        found = _raygun_kodi.addon(id)
        if found is None:
            raise RuntimeError('Unknown addon id \'%s\'.' % id)
        self._id = id or _raygun_kodi.main_id()
        self._info = found

    def getAddonInfo(self, id):
        key = str(id).lower()
        info = self._info
        path = info.get('path', '/kodi/home/addons/' + self._id)
        if key == 'id':
            return self._id
        if key == 'path':
            return path
        if key == 'profile':
            return 'special://profile/addon_data/%s/' % self._id
        if key == 'libpath':
            return path + '/' + info.get('library', '')
        if key in ('icon', 'fanart'):
            value = info.get(key, '')
            return value if not value or value.startswith(('/', 'http')) else path + '/' + value
        if key == 'type':
            return info.get('type', 'xbmc.python.pluginsource')
        if key == 'stars':
            return '0'
        return str(info.get(key, ''))

    def getLocalizedString(self, id):
        return _raygun_kodi.localized(self._id, id)

    def getSetting(self, id):
        return _raygun_kodi.setting(self._id, id)

    def getSettingBool(self, id):
        return self.getSettings().getBool(id)

    def getSettingInt(self, id):
        return self.getSettings().getInt(id)

    def getSettingNumber(self, id):
        return self.getSettings().getNumber(id)

    def getSettingString(self, id):
        return self.getSettings().getString(id)

    def getSettings(self):
        return Settings(self._id)

    def setSetting(self, id, value):
        _raygun_kodi.set_setting(self._id, id, value)

    def setSettingBool(self, id, value):
        _raygun_kodi.set_setting(self._id, id, bool(value))
        return True

    def setSettingInt(self, id, value):
        _raygun_kodi.set_setting(self._id, id, int(value))
        return True

    def setSettingNumber(self, id, value):
        _raygun_kodi.set_setting(self._id, id, float(value))
        return True

    def setSettingString(self, id, value):
        _raygun_kodi.set_setting(self._id, id, str(value))
        return True

    def openSettings(self):
        pass
