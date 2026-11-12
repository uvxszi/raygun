"""Kodi's `xbmcplugin` module, as Raygun runs plugins: what a run lists, resolves and says
about itself is kept for Raygun to read when it ends."""

import _raygun_kodi

SORT_METHOD_NONE = 0
SORT_METHOD_LABEL = 1
SORT_METHOD_LABEL_IGNORE_THE = 2
SORT_METHOD_DATE = 3
SORT_METHOD_SIZE = 4
SORT_METHOD_FILE = 5
SORT_METHOD_DRIVE_TYPE = 6
SORT_METHOD_TRACKNUM = 7
SORT_METHOD_DURATION = 8
SORT_METHOD_TITLE = 9
SORT_METHOD_TITLE_IGNORE_THE = 10
SORT_METHOD_ARTIST = 11
SORT_METHOD_ARTIST_AND_YEAR = 12
SORT_METHOD_ARTIST_IGNORE_THE = 13
SORT_METHOD_ALBUM = 14
SORT_METHOD_ALBUM_IGNORE_THE = 15
SORT_METHOD_GENRE = 16
SORT_METHOD_COUNTRY = 17
SORT_METHOD_YEAR = 18
SORT_METHOD_VIDEO_RATING = 19
SORT_METHOD_VIDEO_USER_RATING = 20
SORT_METHOD_DATEADDED = 21
SORT_METHOD_PROGRAM_COUNT = 22
SORT_METHOD_PLAYLIST_ORDER = 23
SORT_METHOD_EPISODE = 24
SORT_METHOD_VIDEO_TITLE = 25
SORT_METHOD_VIDEO_SORT_TITLE = 26
SORT_METHOD_VIDEO_SORT_TITLE_IGNORE_THE = 27
SORT_METHOD_PRODUCTIONCODE = 28
SORT_METHOD_SONG_RATING = 29
SORT_METHOD_SONG_USER_RATING = 30
SORT_METHOD_MPAA_RATING = 31
SORT_METHOD_VIDEO_RUNTIME = 32
SORT_METHOD_STUDIO = 33
SORT_METHOD_STUDIO_IGNORE_THE = 34
SORT_METHOD_FULLPATH = 35
SORT_METHOD_LABEL_IGNORE_FOLDERS = 36
SORT_METHOD_LASTPLAYED = 37
SORT_METHOD_PLAYCOUNT = 38
SORT_METHOD_LISTENERS = 39
SORT_METHOD_UNSORTED = 40
SORT_METHOD_CHANNEL = 41
SORT_METHOD_CHANNEL_NUMBER = 42
SORT_METHOD_BITRATE = 43
SORT_METHOD_DATE_TAKEN = 44
SORT_METHOD_VIDEO_ORIGINAL_TITLE = 45
SORT_METHOD_VIDEO_ORIGINAL_TITLE_IGNORE_THE = 46

def __getattr__(name):
    # Sort methods Raygun doesn't name (it lists in the plugin's order anyway).
    if name.startswith('SORT_METHOD_'):
        return 0
    raise AttributeError("module 'xbmcplugin' has no attribute '%s'" % name)


def _run(handle):
    return _raygun_kodi.current


def addDirectoryItem(handle, url, listitem=None, isFolder=False, totalItems=0):
    run = _run(handle)
    if len(run.items) >= 5000:
        return False
    run.items.append(_raygun_kodi.item_dict(listitem, url, isFolder))
    return True


def addDirectoryItems(handle, items, totalItems=0):
    for entry in items or []:
        entry = tuple(entry)
        url = entry[0] if entry else ''
        listitem = entry[1] if len(entry) > 1 else None
        folder = entry[2] if len(entry) > 2 else False
        addDirectoryItem(handle, url, listitem, folder)
    return True


def endOfDirectory(handle, succeeded=True, updateListing=False, cacheToDisc=True):
    run = _run(handle)
    run.ended = True
    run.succeeded = bool(succeeded)
    run.cache_to_disc = bool(cacheToDisc)


def setResolvedUrl(handle, succeeded, listitem):
    run = _run(handle)
    data = _raygun_kodi.item_dict(listitem, listitem.getPath() if listitem is not None else '',
                                  False)
    run.resolved = {'ok': bool(succeeded), 'item': data}


def setContent(handle, content):
    _run(handle).content = str(content or '')


def setPluginCategory(handle, category):
    _run(handle).category = _raygun_kodi.clean(category)


def setPluginFanart(handle, image=None, color1=None, color2=None, color3=None):
    if image:
        _run(handle).properties['fanart_image'] = image


def addSortMethod(handle, sortMethod, labelMask='', label2Mask=''):
    _run(handle).sort_methods.append(sortMethod)


def setProperty(handle, key, value):
    _run(handle).properties[str(key)] = str(value)


def getSetting(handle, id):
    return _raygun_kodi.setting(None, id)


def setSetting(handle, id, value):
    _raygun_kodi.set_setting(None, id, value)