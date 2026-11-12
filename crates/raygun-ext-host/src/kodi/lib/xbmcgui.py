"""Kodi's `xbmcgui` module, as Raygun runs plugins: list items (what Raygun reads of each entry),
and dialogs answered without a screen (a choice made beforehand, a search's words, "no")."""

import _raygun_kodi

NOTIFICATION_INFO = 'info'
NOTIFICATION_WARNING = 'warning'
NOTIFICATION_ERROR = 'error'
INPUT_ALPHANUM = 0
INPUT_NUMERIC = 1
INPUT_DATE = 2
INPUT_TIME = 3
INPUT_IPADDRESS = 4
INPUT_PASSWORD = 5
INPUT_TYPE_TEXT = 0
INPUT_TYPE_NUMBER = 1
INPUT_TYPE_SECONDS = 2
INPUT_TYPE_TIME = 3
INPUT_TYPE_DATE = 4
INPUT_TYPE_IPADDRESS = 5
INPUT_TYPE_PASSWORD = 6
INPUT_TYPE_PASSWORD_MD5 = 7
INPUT_TYPE_PASSWORD_NUMBER_VERIFY_NEW = 10
PASSWORD_VERIFY = 1
ALPHANUM_HIDE_INPUT = 2
HORIZONTAL = 0
VERTICAL = 1
DLG_YESNO_NO_BTN = 10
DLG_YESNO_YES_BTN = 11
DLG_YESNO_CUSTOM_BTN = 12
ICON_OVERLAY_NONE = 0
ICON_OVERLAY_RAR = 1
ICON_OVERLAY_ZIP = 2
ICON_OVERLAY_LOCKED = 3
ICON_OVERLAY_UNWATCHED = 4
ICON_OVERLAY_WATCHED = 5
ICON_OVERLAY_HD = 6
ICON_TYPE_SETTINGS = 109
ICON_TYPE_NOTIFICATION = 107
ICON_TYPE_WEATHER = 101
ACTION_PREVIOUS_MENU = 10
ACTION_NAV_BACK = 92
ACTION_SELECT_ITEM = 7
ACTION_MOVE_LEFT = 1
ACTION_MOVE_RIGHT = 2
ACTION_MOVE_UP = 3
ACTION_MOVE_DOWN = 4
ACTION_CONTEXT_MENU = 117
ACTION_SHOW_INFO = 11
ACTION_PARENT_DIR = 9
ACTION_PLAYER_PLAY = 79
ACTION_STOP = 13
ACTION_MOUSE_LEFT_CLICK = 100
KEY_BUTTON_A = 256
CONTROL_TEXT_OFFSET_X = 10
CONTROL_TEXT_OFFSET_Y = 2

# InfoTag setters that name a different info label than their own name.
_INFO_ALIASES = {
    'genres': 'genre', 'countries': 'country', 'directors': 'director', 'writers': 'writer',
    'studios': 'studio', 'tags': 'tag', 'artists': 'artist', 'firstaired': 'aired',
    'tracknumber': 'tracknumber', 'track': 'tracknumber', 'imdbnumber': 'imdbnumber',
    'uniqueids': 'uniqueid', 'showlinks': 'showlink', 'albumartists': 'albumartist',
    'releasedate': 'releasedate', 'trailer': 'trailer', 'tvshowtitle': 'tvshowtitle',
    'resumepoint': 'resume', 'filenameandpath': 'path', 'set': 'set', 'mpaa': 'mpaa',
    'premiered': 'premiered', 'dateadded': 'dateadded', 'lastplayed': 'lastplayed',
}


def _info_method(info, name):
    """`setTitle(value)` and the like of Kodi's InfoTag classes: each records one info label
    (`title`); `getTitle()` reads it back."""
    if name.startswith('set') and len(name) > 3:
        key = name[3:].lower()
        key = _INFO_ALIASES.get(key, key)

        def setter(*args, **kwargs):
            if key == 'cast':
                members = args[0] if args else kwargs.get('actors', [])
                info['cast'] = [actor.getName() if hasattr(actor, 'getName') else str(actor)
                                for actor in members or []]
            elif key == 'resume':
                if args:
                    info['resume'] = args[0]
            elif key == 'rating':
                if args:
                    info['rating'] = args[0]
            else:
                info[key] = args[0] if args else (next(iter(kwargs.values())) if kwargs else '')
        return setter
    if name.startswith('add') and len(name) > 3:
        def adder(*args, **kwargs):
            info.setdefault('_' + name[3:].lower(), []).append(args[0] if args else kwargs)
        return adder
    if name.startswith('get') and len(name) > 3:
        key = name[3:].lower()
        key = _INFO_ALIASES.get(key, key)

        def getter(*args, **kwargs):
            value = info.get(key)
            if value is None:
                return 0 if key in ('year', 'season', 'episode', 'duration', 'playcount',
                                    'tracknumber', 'dbid', 'votes', 'userrating') else ''
            return value
        return getter
    raise AttributeError(name)


class ListItem:
    def __init__(self, label='', label2='', path='', offscreen=False, iconImage=None,
                 thumbnailImage=None):
        self._label = '' if label is None else str(label)
        self._label2 = '' if label2 is None else str(label2)
        self._path = '' if path is None else str(path)
        self._art = {}
        self._info = {}
        self._properties = {}
        self._mime = ''
        self._subtitles = []
        self._streams = []
        self._selected = False
        self._folder = None
        self._content_lookup = True
        if iconImage:
            self._art['icon'] = iconImage
        if thumbnailImage:
            self._art['thumb'] = thumbnailImage

    def _raygun_data(self):
        info = {key: value for key, value in self._info.items() if not key.startswith('_')}
        return {
            'label': _raygun_kodi.clean(self._label),
            'label2': _raygun_kodi.clean(self._label2),
            'path': self._path,
            'art': {key: str(value) for key, value in self._art.items() if value},
            'info': _jsonable(info),
            'properties': {key: str(value) for key, value in self._properties.items()},
            'mime': self._mime,
            'subtitles': [str(item) for item in self._subtitles],
            'isFolder': self._folder,
        }

    def getLabel(self):
        return self._label

    def getLabel2(self):
        return self._label2

    def setLabel(self, label):
        self._label = '' if label is None else str(label)

    def setLabel2(self, label):
        self._label2 = '' if label is None else str(label)

    def getDateTime(self):
        return self._info.get('date', '')

    def setDateTime(self, dateTime):
        self._info['date'] = dateTime

    def setArt(self, values):
        for key, value in (values or {}).items():
            if value:
                self._art[str(key).lower()] = value

    def getArt(self, key):
        return self._art.get(str(key).lower(), '')

    def setIconImage(self, value):
        self._art['icon'] = value

    def setThumbnailImage(self, value):
        self._art['thumb'] = value

    def setAvailableFanart(self, images):
        if images and not self._art.get('fanart'):
            first = images[0]
            self._art['fanart'] = first.get('image') if isinstance(first, dict) else first

    def addAvailableArtwork(self, url, art_type='', preview='', referrer='', cache='',
                            post=False, isgz=False, season=-1):
        if art_type and url and not self._art.get(art_type):
            self._art[art_type] = url

    def isFolder(self):
        return bool(self._folder)

    def setIsFolder(self, isFolder):
        self._folder = bool(isFolder)

    def setUniqueIDs(self, values, defaultrating=''):
        self._info['uniqueid'] = dict(values or {})

    def getUniqueID(self, key):
        return (self._info.get('uniqueid') or {}).get(key, '')

    def setRating(self, type, rating, votes=0, defaultt=False):
        if defaultt or 'rating' not in self._info:
            self._info['rating'] = rating

    def getRating(self, key=''):
        return float(self._info.get('rating') or 0)

    def getVotes(self, key=''):
        return int(self._info.get('votes') or 0)

    def addSeason(self, number, name=''):
        pass

    def select(self, selected):
        self._selected = bool(selected)

    def isSelected(self):
        return self._selected

    def setInfo(self, type='video', infoLabels=None):
        for key, value in (infoLabels or {}).items():
            self._info[str(key).lower()] = value

    def setCast(self, actors):
        self._info['cast'] = [actor.get('name', '') if isinstance(actor, dict) else str(actor)
                              for actor in actors or []]

    def setProperty(self, key, value):
        self._properties[str(key).lower()] = '' if value is None else str(value)

    def setProperties(self, values):
        for key, value in (values or {}).items():
            self.setProperty(key, value)

    def getProperty(self, key):
        return self._properties.get(str(key).lower(), '')

    def addContextMenuItems(self, items, replaceItems=False):
        pass

    def setPath(self, path):
        self._path = '' if path is None else str(path)

    def getPath(self):
        return self._path

    def setMimeType(self, mimetype):
        self._mime = mimetype or ''

    def setContentLookup(self, enable):
        self._content_lookup = bool(enable)

    def setSubtitles(self, subtitleFiles):
        self._subtitles = list(subtitleFiles or [])

    def addStreamInfo(self, type, values):
        self._streams.append((type, dict(values or {})))
        if type == 'video' and values and values.get('duration') and not self._info.get('duration'):
            self._info['duration'] = values.get('duration')

    def getVideoInfoTag(self):
        import xbmc
        return xbmc.InfoTagVideo(_owner=self)

    def getMusicInfoTag(self):
        import xbmc
        return xbmc.InfoTagMusic(_owner=self)

    def getPictureInfoTag(self):
        import xbmc
        return xbmc.InfoTagPicture(_owner=self)

    def getGameInfoTag(self):
        import xbmc
        return xbmc.InfoTagGame(_owner=self)

    def getfilename(self):
        return self._path

    def getduration(self):
        return int(self._info.get('duration') or 0)

    def setInfoTag(self, *args, **kwargs):
        pass


def _jsonable(value):
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    if isinstance(value, dict):
        return {str(key): _jsonable(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_jsonable(item) for item in value]
    if hasattr(value, 'getName'):
        return value.getName()
    return str(value)


class Dialog:
    def ok(self, heading, message='', *args, **kwargs):
        _raygun_kodi.message(message or heading)
        return True

    def yesno(self, heading, message='', nolabel='', yeslabel='', autoclose=0,
              defaultbutton=DLG_YESNO_NO_BTN, *args, **kwargs):
        return _raygun_kodi.current.yesno

    def yesnocustom(self, heading, message='', customlabel='', nolabel='', yeslabel='',
                    autoclose=0, defaultbutton=DLG_YESNO_NO_BTN):
        return 1 if _raygun_kodi.current.yesno else 0

    def info(self, item):
        return False

    def select(self, heading, list=(), autoclose=0, preselect=-1, useDetails=False):
        run = _raygun_kodi.current
        options = [_raygun_kodi.clean(option.getLabel() if isinstance(option, ListItem)
                                      else option) for option in list]
        if run.select_answers:
            answer = run.select_answers.pop(0)
            if isinstance(answer, int) and 0 <= answer < len(options):
                return answer
        if not options:
            return -1
        run.selects.append({'heading': _raygun_kodi.clean(heading), 'options': options})
        if run.probe and len(run.selects) == 1:
            return -1
        return preselect if 0 <= preselect < len(options) else 0

    def multiselect(self, heading, options, autoclose=0, preselect=None, useDetails=False):
        return None

    def contextmenu(self, list):
        return -1

    def input(self, heading, defaultt='', type=INPUT_ALPHANUM, option=0, autoclose=0):
        answer = _raygun_kodi.current.keyboard
        return defaultt if answer is None else answer

    def numeric(self, type, heading, defaultt='', bHiddenInput=False):
        return defaultt

    def browse(self, type, heading, shares, mask='', useThumbs=False, treatAsFolder=False,
               defaultt='', enableMultiple=False):
        return defaultt

    def browseSingle(self, type, heading, shares, mask='', useThumbs=False,
                     treatAsFolder=False, defaultt=''):
        return defaultt

    def browseMultiple(self, type, heading, shares, mask='', useThumbs=False,
                       treatAsFolder=False, defaultt=''):
        return []

    def notification(self, heading, message, icon=NOTIFICATION_INFO, time=5000, sound=True):
        _raygun_kodi.message(message or heading)

    def textviewer(self, heading, text, usemono=False):
        _raygun_kodi.message(text or heading)

    def colorpicker(self, heading, selectedcolor='', colorfile='', colorlist=None):
        return selectedcolor


class DialogProgress:
    def create(self, heading, message='', *args, **kwargs):
        pass

    def update(self, percent, message='', *args, **kwargs):
        pass

    def iscanceled(self):
        return False

    def close(self):
        pass


class DialogProgressBG:
    def create(self, heading, message=''):
        pass

    def update(self, percent=0, heading='', message=''):
        pass

    def isFinished(self):
        return True

    def close(self):
        pass


class DialogBusy:
    def create(self):
        pass

    def update(self, percent):
        pass

    def iscanceled(self):
        return False

    def close(self):
        pass


class Window:
    def __init__(self, existingWindowId=-1):
        self._id = existingWindowId

    def show(self):
        pass

    def close(self):
        pass

    def doModal(self):
        pass

    def onAction(self, action):
        pass

    def onClick(self, controlId):
        pass

    def onFocus(self, controlId):
        pass

    def onControl(self, control):
        pass

    def onInit(self):
        pass

    def setProperty(self, key, value):
        _raygun_kodi.window_properties[str(key).lower()] = '' if value is None else str(value)

    def getProperty(self, key):
        return _raygun_kodi.window_properties.get(str(key).lower(), '')

    def clearProperty(self, key):
        _raygun_kodi.window_properties.pop(str(key).lower(), None)

    def clearProperties(self):
        _raygun_kodi.window_properties.clear()

    def getFocusId(self):
        return 0

    def getFocus(self):
        return None

    def setFocus(self, control):
        pass

    def setFocusId(self, controlId):
        pass

    def getControl(self, controlId):
        return Control()

    def addControl(self, control):
        pass

    def addControls(self, controls):
        pass

    def removeControl(self, control):
        pass

    def removeControls(self, controls):
        pass

    def getHeight(self):
        return 720

    def getWidth(self):
        return 1280

    def getResolution(self):
        return 0

    def setCoordinateResolution(self, resolution):
        pass


class WindowDialog(Window):
    pass


class WindowXML(Window):
    def __init__(self, xmlFilename='', scriptPath='', defaultSkin='Default', defaultRes='720p',
                 isMedia=False):
        super().__init__()

    def addItem(self, item, position=2147483647):
        pass

    def addItems(self, items):
        pass

    def clearList(self):
        pass

    def getCurrentListPosition(self):
        return 0

    def getListItem(self, position):
        return ListItem()

    def getListSize(self):
        return 0

    def setContainerProperty(self, key, value):
        pass

    def setContent(self, value):
        pass

    def getCurrentContainerId(self):
        return 0


class WindowXMLDialog(WindowXML):
    pass


class Action:
    def getId(self):
        return 0

    def getButtonCode(self):
        return 0

    def getAmount1(self):
        return 0.0

    def getAmount2(self):
        return 0.0


class Control:
    def __init__(self, *args, **kwargs):
        pass

    def __getattr__(self, name):
        def nothing(*args, **kwargs):
            return None
        return nothing


ControlLabel = ControlFadeLabel = ControlTextBox = ControlButton = ControlEdit = Control
ControlList = ControlImage = ControlProgress = ControlSlider = ControlGroup = Control
ControlRadioButton = ControlSpin = ControlCheckMark = Control


def getCurrentWindowId():
    return 10025


def getCurrentWindowDialogId():
    return 9999


def getScreenHeight():
    return 1080


def getScreenWidth():
    return 1920
