"""The state of one plugin run, shared by Raygun's `xbmc*` modules: the request Raygun made (the
plugin's address, its handle, the answers to dialogs), and what the plugin did (the items it
listed, the stream it resolved, the dialogs it opened). One run at a time per interpreter."""

import _raygun

KODI_VERSION = '21.2.0'
KODI_BUILD = '21.2 (21.2.0) Git:20250301-raygun'


class Run:
    def __init__(self, request):
        self.request = request
        self.url = request.get('url', '')
        self.handle = int(request.get('handle', 1))
        self.items = []
        self.ended = False
        self.succeeded = True
        self.content = ''
        self.category = ''
        self.resolved = None
        self.redirect = None
        self.played = None
        self.select_answers = list(request.get('select') or [])
        self.probe = bool(request.get('probe'))
        self.keyboard = request.get('keyboard')
        self.yesno = bool(request.get('yesno', False))
        self.selects = []
        self.messages = []
        self.settings_changed = {}
        self.drm = None
        self.sort_methods = []
        self.properties = {}
        self.cache_to_disc = True


current = Run({})
addons = {}
language = 'en_gb'
platform = 'android'
window_properties = {}
_strings = {}


def start(request):
    global current, addons, language, platform
    current = Run(request)
    addons = request.get('addons') or {}
    language = (request.get('language') or 'en_gb').lower()
    platform = request.get('platform') or 'android'
    return current


def main_id():
    return current.request.get('addon') or next(iter(addons), '')


def addon(addon_id=None):
    if not addon_id:
        addon_id = main_id()
    return addons.get(addon_id)


def setting(addon_id, key):
    info = addon(addon_id) or {}
    value = (info.get('settings') or {}).get(key)
    return '' if value is None else str(value)


def set_setting(addon_id, key, value):
    info = addon(addon_id)
    if info is None:
        return
    text = str(value)
    if isinstance(value, bool):
        text = 'true' if value else 'false'
    info.setdefault('settings', {})[key] = text
    current.settings_changed.setdefault(addon_id, {})[key] = text


def note_drm(drm):
    current.drm = str(drm)


def message(text):
    text = clean(str(text or '')).strip()
    if text and text not in current.messages:
        current.messages.append(text[:500])


def clean(text):
    """A Kodi label without its formatting tags (`[B]`, `[COLOR red]`, `[CR]`...)."""
    import re
    if not isinstance(text, str):
        text = '' if text is None else str(text)
    text = re.sub(r'\[CR\]', ' ', text, flags=re.I)
    text = re.sub(r'\[/?(B|I|LIGHT|UPPERCASE|LOWERCASE|CAPITALIZE|TABS)\]', '', text, flags=re.I)
    text = re.sub(r'\[COLOR[^\]]*\]|\[/COLOR\]', '', text, flags=re.I)
    return re.sub(r'\s+', ' ', text).strip()


def localized(addon_id, number):
    """A string of an add-on's `strings.po` (or `strings.xml`), in Raygun's language if it has
    them, else English."""
    if not addon_id:
        addon_id = main_id()
    table = _strings.get(addon_id)
    if table is None:
        table = _load_strings(addon_id)
        _strings[addon_id] = table
    return table.get(int(number), '')


def _load_strings(addon_id):
    info = addon(addon_id) or {}
    root = info.get('path') or ('/kodi/home/addons/' + addon_id)
    candidates = []
    for name in (language, language.split('_')[0] + '_' + language.split('_')[0], 'en_gb',
                 'en_us', 'en_au', 'en_nz'):
        candidates.append('%s/resources/language/resource.language.%s/strings.po' % (root, name))
    candidates.append('%s/resources/language/English/strings.po' % root)
    candidates.append('%s/resources/language/English/strings.xml' % root)
    for path in candidates:
        try:
            data = _raygun.fs_read(path).decode('utf-8', 'replace')
        except OSError:
            continue
        if path.endswith('.xml'):
            return _parse_xml_strings(data)
        return _parse_po(data)
    return {}


def _unquote_po(line):
    line = line.strip()
    if line.startswith('"') and line.endswith('"'):
        line = line[1:-1]
    return (line.replace('\\n', '\n').replace('\\t', '\t').replace('\\"', '"')
            .replace('\\\\', '\\'))


def _parse_po(text):
    table = {}
    number = None
    field = None
    values = {'msgid': '', 'msgstr': ''}

    def flush():
        if number is not None:
            found = values['msgstr'] or values['msgid']
            if found or number not in table:
                table[number] = found

    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith('msgctxt'):
            flush()
            values = {'msgid': '', 'msgstr': ''}
            field = None
            context = _unquote_po(line[7:])
            number = int(context[1:]) if context.startswith('#') and context[1:].isdigit() else None
        elif line.startswith('msgid'):
            field = 'msgid'
            values['msgid'] = _unquote_po(line[5:])
        elif line.startswith('msgstr'):
            field = 'msgstr'
            values['msgstr'] = _unquote_po(line[6:])
        elif line.startswith('"') and field:
            values[field] += _unquote_po(line)
    flush()
    return table


def _parse_xml_strings(text):
    import re
    import html
    table = {}
    for match in re.finditer(r'<string\s+id="(\d+)"\s*>(.*?)</string>', text, re.S):
        table[int(match.group(1))] = html.unescape(match.group(2))
    return table


def item_dict(listitem, url, folder):
    """What Raygun keeps of a listed item."""
    if listitem is None:
        import xbmcgui
        listitem = xbmcgui.ListItem()
    data = listitem._raygun_data() if hasattr(listitem, '_raygun_data') else {}
    data['url'] = url if isinstance(url, str) else ('' if url is None else str(url))
    data['folder'] = bool(folder)
    return data
