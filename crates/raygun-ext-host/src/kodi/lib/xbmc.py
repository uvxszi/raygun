"""Kodi's `xbmc` module, as Raygun runs plugins: logging, info labels, the player and playlists
(what a plugin plays is handed back to Raygun), keyboards answered with what was searched for."""

import json as _json
import re as _re
import time as _time

import _raygun
import _raygun_kodi

LOGDEBUG = 0
LOGINFO = 1
LOGNOTICE = 1
LOGWARNING = 2
LOGERROR = 3
LOGSEVERE = 4
LOGFATAL = 4
LOGNONE = 5

PLAYLIST_MUSIC = 0
PLAYLIST_VIDEO = 1
PLAYER_CORE_AUTO = 0
PLAYER_CORE_DVDPLAYER = 1
PLAYER_CORE_MPLAYER = 2
PLAYER_CORE_PAPLAYER = 3
TRAY_OPEN = 16
DRIVE_NOT_READY = 1
TRAY_CLOSED_NO_MEDIA = 64
TRAY_CLOSED_MEDIA_PRESENT = 96
SERVER_WEBSERVER = 1
SERVER_AIRPLAYSERVER = 2
SERVER_UPNPSERVER = 3
SERVER_UPNPRENDERER = 4
SERVER_EVENTSERVER = 5
SERVER_JSONRPCSERVER = 6
SERVER_ZEROCONF = 7
ISO_639_1 = 0
ISO_639_2 = 1
ENGLISH_NAME = 2
CAPTURE_STATE_WORKING = 0
CAPTURE_STATE_DONE = 1
CAPTURE_STATE_FAILED = 2
CAPTURE_FLAG_CONTINUOUS = 1
CAPTURE_FLAG_IMMEDIATELY = 2

abortRequested = False

# Kodi's own strings plugins ask for by number (`getLocalizedString`).
_KODI_STRINGS = {
    106: 'No', 107: 'Yes', 137: 'Search', 186: 'OK', 222: 'Cancel', 283: 'Search results',
    369: 'Title', 515: 'Genre', 552: 'Date', 562: 'Year', 563: 'Rating', 570: 'Date added',
    20334: 'Movies', 20343: 'TV shows', 20359: 'Episodes', 20373: 'Season', 20359 + 1: 'Episode',
    33063: 'Next page', 33078: 'Next page', 33077: 'Previous page', 19020: 'Channels',
    15016: 'Games', 20342: 'Movies', 1037: 'Video add-ons', 1038: 'Music add-ons',
    19029: 'Channels', 30000: '', 13208: 'Alphabetical', 13385: 'Clear', 24026: 'Update',
}


def log(msg, level=LOGDEBUG):
    if isinstance(msg, bytes):
        msg = msg.decode('utf-8', 'replace')
    _raygun.log(int(level) if isinstance(level, int) else LOGDEBUG, str(msg))


def output(msg, level=LOGDEBUG):
    log(msg, level)


def sleep(time):
    _raygun.sleep(max(0, int(time)) / 1000.0)


def getLanguage(format=ENGLISH_NAME, region=False):
    if format == ISO_639_1:
        return 'en-gb' if region else 'en'
    if format == ISO_639_2:
        return 'eng'
    return 'English'


def getRegion(id):
    return {
        'datelong': '%A, %d %B %Y', 'dateshort': '%d/%m/%Y', 'tempunit': '°C',
        'speedunit': 'km/h', 'time': '%H:%M:%S', 'meridiem': 'AM/PM',
    }.get(id, '')


def getLocalizedString(id):
    return _KODI_STRINGS.get(int(id), '')


def getSkinDir():
    return 'skin.estuary'


def getUserAgent():
    return 'Kodi/%s (Linux; Android 14; Raygun) Android/14.0.0 Sys_CPU/aarch64 App_Bitness/64 Version/%s' % (
        _raygun_kodi.KODI_VERSION, _raygun_kodi.KODI_VERSION)


def getIPAddress():
    return ''


def getFreeMem():
    return 2048


def getDVDState():
    return DRIVE_NOT_READY


def getGlobalIdleTime():
    return 0


def getCacheThumbName(path):
    import zlib
    return '%08x.tbn' % (zlib.crc32(path.lower().encode('utf-8')) & 0xffffffff)


def makeLegalFilename(filename, fatX=True):
    return _re.sub(r'[\\:*?"<>|]', '_', filename)


def validatePath(path):
    return path


def translatePath(path):
    import xbmcvfs
    return xbmcvfs.translatePath(path)


def getSupportedMedia(media):
    return {
        'video': '.m4v|.3g2|.3gp|.nsv|.tp|.ts|.ty|.strm|.pls|.rm|.rmvb|.mpd|.m3u|.m3u8|.ifo|.mov|.qt|.divx|.xvid|.bivx|.vob|.nrg|.img|.iso|.udf|.pva|.wmv|.asf|.asx|.ogm|.m2v|.avi|.bin|.dat|.mpg|.mpeg|.mp4|.mkv|.mk3d|.avc|.vp3|.svq3|.nuv|.viv|.dv|.fli|.flv|.001|.wpl|.xspf|.zip|.vdr|.dvr-ms|.xsp|.mts|.m2t|.m2ts|.evo|.ogv|.sdp|.avs|.rec|.url|.pxml|.vc1|.h264|.rcv|.rss|.mpls|.mpl|.webm|.bdmv|.bdm|.wtv|.trp|.f4v',
        'music': '.nsv|.m4a|.flac|.aac|.strm|.pls|.rm|.rma|.mpa|.wav|.wma|.ogg|.mp3|.mp2|.m3u|.gdm|.imf|.m15|.sfx|.uni|.ac3|.dts|.cue|.aif|.aiff|.wpl|.xspf|.ape|.mac|.mpc|.mp+|.mpp|.shn|.zip|.wv|.dsp|.xsp|.xwav|.waa|.wvs|.wam|.gcm|.idsp|.mpdsp|.mss|.spt|.rsd|.sap|.cmc|.cmr|.dmc|.mpt|.mpd|.rmt|.tmc|.tm8|.tm2|.oga|.url|.pxml|.tta|.rss|.wtv|.mka|.tak|.opus|.dff|.dsf|.m4b|.dtshd',
        'picture': '.png|.jpg|.jpeg|.bmp|.gif|.ico|.tif|.tiff|.tga|.pcx|.cbz|.zip|.rss|.webp|.jp2|.apng',
    }.get(media, '')


def skinHasImage(image):
    return False


def startServer(iTyp, bStart, bWait=False):
    return False


def audioSuspend():
    pass


def audioResume():
    pass


def enableNavSounds(yesNo):
    pass


def playSFX(filename, useCached=True):
    pass


def stopSFX():
    pass


def restart():
    pass


def shutdown():
    pass


def executescript(script):
    log('executescript(%s) ignored in Raygun' % script, LOGINFO)


def getInfoImage(infotag):
    return ''


def convertLanguage(language, format):
    names = {'en': ('en', 'eng', 'English'), 'english': ('en', 'eng', 'English'),
             'de': ('de', 'ger', 'German'), 'fr': ('fr', 'fre', 'French'),
             'es': ('es', 'spa', 'Spanish'), 'it': ('it', 'ita', 'Italian'),
             'nl': ('nl', 'dut', 'Dutch'), 'sv': ('sv', 'swe', 'Swedish')}
    found = names.get(str(language).lower()[:2]) or names.get(str(language).lower())
    if not found:
        return language
    return found[format if format in (0, 1, 2) else 2]


def _plugin_id():
    return _raygun_kodi.main_id()


def getInfoLabel(cLine):
    label = str(cLine)
    key = label.lower()
    if key == 'system.buildversion':
        return _raygun_kodi.KODI_BUILD
    if key == 'system.builddate':
        return '2025-03-01'
    if key in ('system.language', 'system.locale.language'):
        return 'English'
    if key == 'system.profilename':
        return 'Master user'
    if key == 'system.friendlyname':
        return 'Raygun'
    if key == 'container.folderpath':
        return _raygun_kodi.current.url
    if key == 'container.pluginname':
        return _plugin_id()
    if key == 'system.date':
        return _time.strftime('%A, %d %B %Y')
    if key == 'system.time':
        return _time.strftime('%H:%M')
    if key == 'system.screenwidth':
        return '1920'
    if key == 'system.screenheight':
        return '1080'
    if key == 'system.screenresolution':
        return '1920x1080 @ 60.00Hz - Full screen'
    if key.startswith('system.addonversion(') and key.endswith(')'):
        found = _raygun_kodi.addon(label[20:-1])
        return (found or {}).get('version', '')
    if key.startswith('system.addontitle(') and key.endswith(')'):
        found = _raygun_kodi.addon(label[18:-1])
        return (found or {}).get('name', '')
    if key == 'system.osversioninfo':
        return 'Android 14' if _raygun_kodi.platform == 'android' else 'Windows 10'
    if key == 'network.ipaddress':
        return ''
    if key.startswith('window(') and '.property(' in key:
        inner = label[label.lower().index('.property(') + 10:-1]
        return _raygun_kodi.window_properties.get(inner.lower(), '')
    return ''


def _has_addon(addon_id):
    addon_id = addon_id.strip().strip('"\'').lower()
    if addon_id in ('inputstream.adaptive', 'inputstream.ffmpegdirect', 'inputstream.rtmp',
                    'script.module.inputstreamhelper', 'xbmc.python', 'xbmc.addon'):
        return True
    return any(known.lower() == addon_id for known in _raygun_kodi.addons)


def getCondVisibility(condition):
    text = str(condition).strip()
    lowered = text.lower()
    if lowered.startswith('!'):
        return not getCondVisibility(text[1:])
    if '+' in lowered:
        return all(getCondVisibility(part) for part in text.split('+'))
    if '|' in lowered:
        return any(getCondVisibility(part) for part in text.split('|'))
    if lowered == 'system.platform.android':
        return _raygun_kodi.platform == 'android'
    if lowered == 'system.platform.windows':
        return _raygun_kodi.platform == 'windows'
    if lowered == 'system.platform.linux':
        return _raygun_kodi.platform in ('android', 'linux')
    if lowered.startswith('system.platform.'):
        return False
    match = _re.match(r'system\.(hasaddon|addonisenabled)\((.+)\)$', lowered)
    if match:
        return _has_addon(match.group(2))
    if lowered.startswith('system.hasmediadvd') or lowered.startswith('player.'):
        return False
    if lowered.startswith('window.isvisible') or lowered.startswith('window.isactive'):
        return False
    if lowered in ('system.isfullscreen',):
        return True
    if lowered.startswith('system.getbool('):
        return False
    return False


def _redirect_target(argument):
    url = argument.split(',')[0].strip().strip('"\'')
    return url if url.startswith('plugin://') else None


def executebuiltin(function, wait=False):
    text = str(function).strip()
    lowered = text.lower()
    run = _raygun_kodi.current
    name, _, rest = text.partition('(')
    argument = rest[:-1] if rest.endswith(')') else rest
    name = name.strip().lower()
    if name in ('container.update', 'activatewindow', 'replacewindow'):
        if name == 'container.update':
            target = _redirect_target(argument)
        else:
            parts = [part.strip() for part in argument.split(',')]
            target = next((part.strip('"\'') for part in parts
                           if part.strip('"\'').startswith('plugin://')), None)
        if target:
            run.redirect = target
        return
    if name == 'playmedia':
        url = argument.split(',')[0].strip().strip('"\'')
        if url:
            import xbmcgui
            run.played = {'items': [_raygun_kodi.item_dict(xbmcgui.ListItem(path=url), url,
                                                             False)]}
        return
    if name == 'notification':
        parts = [part.strip() for part in argument.split(',')]
        _raygun_kodi.message(': '.join(part for part in parts[:2] if part))
        return
    log('executebuiltin(%s) ignored in Raygun' % text, LOGDEBUG)
    del lowered


_JSON_ANSWERS = {
    'JSONRPC.Version': {'version': {'major': 13, 'minor': 5, 'patch': 0}},
    'Application.GetProperties': {
        'version': {'major': 21, 'minor': 2, 'revision': '', 'tag': 'stable'},
        'name': 'Kodi', 'language': 'en_GB', 'volume': 100, 'muted': False},
}


def _setting_value(name):
    return {
        'locale.language': 'resource.language.en_gb',
        'locale.country': 'United Kingdom',
        'locale.audiolanguage': 'original',
        'locale.subtitlelanguage': 'original',
        'locale.timezone': 'UTC',
        'network.usehttpproxy': False,
        'videoplayer.preferdefaultflag': True,
        'subtitles.languages': ['English'],
        'debug.showloginfo': False,
        'services.webserver': False,
        'filelists.showparentdiritems': False,
        'videolibrary.showunwatchedplots': [0, 1, 2],
        'audiooutput.passthrough': False,
    }.get(name)


def executeJSONRPC(jsonrpccommand):
    try:
        request = _json.loads(jsonrpccommand)
    except ValueError:
        return _json.dumps({'jsonrpc': '2.0', 'error': {'code': -32700, 'message': 'Parse error.'}})
    method = request.get('method', '')
    params = request.get('params') or {}
    ident = request.get('id', 1)
    result = _JSON_ANSWERS.get(method)
    if method == 'Addons.GetAddonDetails':
        addon_id = params.get('addonid', '')
        if _has_addon(addon_id):
            info = _raygun_kodi.addon(addon_id) or {}
            result = {'addon': {'addonid': addon_id, 'enabled': True, 'installed': True,
                                'version': info.get('version', '21.4.4'), 'type': 'xbmc.addon',
                                'name': info.get('name', addon_id)}}
        else:
            return _json.dumps({'jsonrpc': '2.0', 'id': ident,
                                'error': {'code': -32602, 'message': 'Invalid params.'}})
    elif method == 'Addons.GetAddons':
        result = {'addons': [{'addonid': key, 'type': 'xbmc.python.module', 'enabled': True}
                             for key in _raygun_kodi.addons], 'limits': {}}
    elif method == 'Addons.SetAddonEnabled':
        result = 'OK'
    elif method == 'Settings.GetSettingValue':
        result = {'value': _setting_value(params.get('setting', ''))}
    elif method == 'Settings.SetSettingValue':
        result = True
    elif method == 'Player.GetActivePlayers':
        result = []
    elif method in ('Files.GetDirectory', 'VideoLibrary.GetMovies', 'VideoLibrary.GetTVShows',
                    'VideoLibrary.GetEpisodes', 'AudioLibrary.GetSongs'):
        result = {'files': [], 'movies': [], 'tvshows': [], 'episodes': [], 'songs': [],
                  'limits': {'start': 0, 'end': 0, 'total': 0}}
    elif method.startswith('GUI.') or method.startswith('Input.'):
        result = 'OK'
    if result is None:
        return _json.dumps({'jsonrpc': '2.0', 'id': ident,
                            'error': {'code': -32601, 'message': 'Method not found.'}})
    return _json.dumps({'jsonrpc': '2.0', 'id': ident, 'result': result})


class Monitor:
    def __init__(self, *args, **kwargs):
        pass

    def abortRequested(self):
        return False

    def waitForAbort(self, timeout=None):
        _raygun.sleep(min(float(timeout), 5.0) if timeout else 0.0)
        return False

    def onSettingsChanged(self):
        pass

    def onScreensaverActivated(self):
        pass

    def onScreensaverDeactivated(self):
        pass

    def onNotification(self, sender, method, data):
        pass

    def onScanStarted(self, library):
        pass

    def onScanFinished(self, library):
        pass

    def onCleanStarted(self, library):
        pass

    def onCleanFinished(self, library):
        pass

    def onDPMSActivated(self):
        pass

    def onDPMSDeactivated(self):
        pass


class Keyboard:
    def __init__(self, line='', heading='', hidden=False):
        self._text = line or ''
        self._heading = heading
        self._confirmed = False

    def doModal(self, autoclose=0):
        answer = _raygun_kodi.current.keyboard
        if answer is None:
            self._confirmed = False
            return
        self._text = answer
        self._confirmed = True

    def setDefault(self, line=''):
        self._text = line

    def setHeading(self, heading):
        self._heading = heading

    def setHiddenInput(self, hidden=False):
        pass

    def getText(self):
        return self._text

    def isConfirmed(self):
        return self._confirmed


class InfoTagVideo:
    def __init__(self, offscreen=False, _owner=None):
        self._info = {} if _owner is None else _owner._info

    def __getattr__(self, name):
        from xbmcgui import _info_method
        return _info_method(self._info, name)


class InfoTagMusic(InfoTagVideo):
    pass


class InfoTagPicture(InfoTagVideo):
    pass


class InfoTagGame(InfoTagVideo):
    pass


class InfoTagRadioRDS(InfoTagVideo):
    pass


class Actor:
    def __init__(self, name='', role='', order=-1, thumbnail=''):
        self._name = name
        self._role = role
        self._order = order
        self._thumbnail = thumbnail

    def getName(self):
        return self._name

    def getRole(self):
        return self._role

    def getOrder(self):
        return self._order

    def getThumbnail(self):
        return self._thumbnail

    def setName(self, name):
        self._name = name

    def setRole(self, role):
        self._role = role

    def setOrder(self, order):
        self._order = order

    def setThumbnail(self, thumbnail):
        self._thumbnail = thumbnail


class _StreamDetail:
    def __init__(self, **kwargs):
        self.__dict__.update(kwargs)

    def __getattr__(self, name):
        if name.startswith('set'):
            return lambda value: self.__dict__.__setitem__(name[3:].lower(), value)
        if name.startswith('get'):
            return lambda: self.__dict__.get(name[3:].lower())
        raise AttributeError(name)


class VideoStreamDetail(_StreamDetail):
    def __init__(self, width=0, height=0, aspect=0.0, duration=0, codec='', stereomode='',
                 language='', hdrtype=''):
        super().__init__(width=width, height=height, aspect=aspect, duration=duration,
                         codec=codec, stereomode=stereomode, language=language, hdrtype=hdrtype)


class AudioStreamDetail(_StreamDetail):
    def __init__(self, channels=-1, codec='', language=''):
        super().__init__(channels=channels, codec=codec, language=language)


class SubtitleStreamDetail(_StreamDetail):
    def __init__(self, language=''):
        super().__init__(language=language)


class PlayList:
    _lists = {}

    def __new__(cls, playList=PLAYLIST_VIDEO):
        found = cls._lists.get(playList)
        if found is None:
            found = super().__new__(cls)
            found._id = playList
            found._items = []
            found._position = -1
            cls._lists[playList] = found
        return found

    def __init__(self, playList=PLAYLIST_VIDEO):
        pass

    def getPlayListId(self):
        return self._id

    def add(self, url, listitem=None, index=-1):
        import xbmcgui
        if isinstance(url, xbmcgui.ListItem):
            listitem, url = url, url.getPath()
        if listitem is None:
            listitem = xbmcgui.ListItem(path=url)
        elif url and not listitem.getPath():
            listitem.setPath(url)
        entry = (url, listitem)
        if index is None or index < 0 or index >= len(self._items):
            self._items.append(entry)
        else:
            self._items.insert(index, entry)

    def load(self, filename):
        import xbmcgui
        try:
            with open(filename, encoding='utf-8', errors='replace') as handle:
                text = handle.read()
        except OSError:
            return False
        urls = []
        for line in text.splitlines():
            line = line.strip()
            if not line or line.startswith('#') or line.startswith('['):
                continue
            if '=' in line and line.lower().startswith('file'):
                urls.append(line.split('=', 1)[1].strip())
            elif '://' in line:
                urls.append(line)
        for url in urls:
            self._items.append((url, xbmcgui.ListItem(path=url)))
        return True

    def remove(self, filename):
        self._items = [entry for entry in self._items if entry[0] != filename]

    def clear(self):
        self._items = []
        self._position = -1

    def size(self):
        return len(self._items)

    def __len__(self):
        return len(self._items)

    def __getitem__(self, index):
        return self._items[index][1]

    def shuffle(self):
        pass

    def unshuffle(self):
        pass

    def getposition(self):
        return self._position


class Player:
    def __init__(self, *args, **kwargs):
        pass

    def play(self, item='', listitem=None, windowed=False, startpos=-1):
        import xbmcgui
        run = _raygun_kodi.current
        entries = []
        if isinstance(item, PlayList):
            entries = [_raygun_kodi.item_dict(entry[1], entry[0], False) for entry in item._items]
            if startpos and 0 < startpos < len(entries):
                entries = entries[startpos:]
        elif isinstance(item, xbmcgui.ListItem):
            entries = [_raygun_kodi.item_dict(item, item.getPath(), False)]
        elif item:
            listitem = listitem or xbmcgui.ListItem(path=item)
            if not listitem.getPath():
                listitem.setPath(item)
            entries = [_raygun_kodi.item_dict(listitem, item, False)]
        else:
            playlist = PlayList(PLAYLIST_VIDEO)
            if not playlist.size():
                playlist = PlayList(PLAYLIST_MUSIC)
            entries = [_raygun_kodi.item_dict(entry[1], entry[0], False)
                       for entry in playlist._items]
        if entries:
            run.played = {'items': entries}

    def stop(self):
        pass

    def pause(self):
        pass

    def playnext(self):
        pass

    def playprevious(self):
        pass

    def playselected(self, selected):
        pass

    def isPlaying(self):
        return False

    def isPlayingAudio(self):
        return False

    def isPlayingVideo(self):
        return False

    def isPlayingRDS(self):
        return False

    def isExternalPlayer(self):
        return False

    def getPlayingFile(self):
        return ''

    def getPlayingItem(self):
        import xbmcgui
        return xbmcgui.ListItem()

    def getTime(self):
        return 0.0

    def getTotalTime(self):
        return 0.0

    def seekTime(self, seekTime):
        pass

    def setSubtitles(self, subtitleFile):
        pass

    def showSubtitles(self, bVisible):
        pass

    def getSubtitles(self):
        return ''

    def getAvailableSubtitleStreams(self):
        return []

    def setSubtitleStream(self, iStream):
        pass

    def getAvailableAudioStreams(self):
        return []

    def setAudioStream(self, iStream):
        pass

    def getAvailableVideoStreams(self):
        return []

    def setVideoStream(self, iStream):
        pass

    def getVideoInfoTag(self):
        return InfoTagVideo()

    def getMusicInfoTag(self):
        return InfoTagMusic()

    def updateInfoTag(self, item):
        pass

    def getRadioRDSInfoTag(self):
        return InfoTagRadioRDS()

    def onPlayBackStarted(self):
        pass

    def onAVStarted(self):
        pass

    def onAVChange(self):
        pass

    def onPlayBackEnded(self):
        pass

    def onPlayBackStopped(self):
        pass

    def onPlayBackError(self):
        pass

    def onPlayBackPaused(self):
        pass

    def onPlayBackResumed(self):
        pass

    def onQueueNextItem(self):
        pass

    def onPlayBackSpeedChanged(self, speed):
        pass

    def onPlayBackSeek(self, time, seekOffset):
        pass

    def onPlayBackSeekChapter(self, chapter):
        pass


class RenderCapture:
    def capture(self, width, height):
        pass

    def getAspectRatio(self):
        return 1.78

    def getImage(self, msecs=0):
        return bytearray()

    def getWidth(self):
        return 0

    def getHeight(self):
        return 0
