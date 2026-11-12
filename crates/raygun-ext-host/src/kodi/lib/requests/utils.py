"""`requests.utils`: the helpers add-ons use."""

import urllib.parse

from .cookies import add_dict_to_cookiejar, cookiejar_from_dict, dict_from_cookiejar  # noqa: F401
from .structures import CaseInsensitiveDict  # noqa: F401

quote = urllib.parse.quote
unquote = urllib.parse.unquote
urlparse = urllib.parse.urlparse


def requote_uri(uri):
    return urllib.parse.quote(urllib.parse.unquote(uri), safe="!#$%&'()*+,/:;=?@[]~")


def default_user_agent(name='python-requests'):
    return '%s/2.31.0' % name


def default_headers():
    from . import Session
    return CaseInsensitiveDict(Session().headers)


def get_encoding_from_headers(headers):
    from . import _encoding_of
    return _encoding_of(headers.get('content-type'))


def get_encodings_from_content(content):
    import re
    found = re.findall(r'<meta.*?charset=["\']?([\w-]+)', content, flags=re.I)
    return found


def parse_header_links(value):
    links = []
    for part in value.split(','):
        url, _, params = part.partition(';')
        link = {'url': url.strip(' <>')}
        for param in params.split(';'):
            key, _, val = param.partition('=')
            if key.strip():
                link[key.strip()] = val.strip(' "\'')
        links.append(link)
    return links


def to_key_val_list(value):
    if value is None:
        return None
    if hasattr(value, 'items'):
        value = value.items()
    return list(value)


def guess_filename(obj):
    name = getattr(obj, 'name', None)
    if isinstance(name, str) and name[0] != '<' and name[-1] != '>':
        return name.rsplit('/', 1)[-1]
    return None
