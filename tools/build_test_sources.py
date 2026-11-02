"""Builds Raygun's test sources (M46.17) into repo-test/: never shipped, never pre-added.

Settings → Data → Developer → "Add test sources" adds them, with the known repositories and some
legal Stremio addons, to see how the app behaves with many sources and with broken ones:

- repo-test/index.json + packages/: extensions that fail on purpose (they throw, hang, return
  garbage, crash while loading, hand out dead streams), to check that every failure stays calm;
- repo-test/stremio/: a static Stremio addon (plain JSON files) of legal films and live news
  channels, each film listing dead links before working ones, so automatic fallback is tested.

Run: python -I tools/build_test_sources.py (from the repository root).
"""

import hashlib
import io
import json
import os
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, 'repo-test')

# ---- Extensions that fail on purpose ----

EXTENSIONS = [
    {
        'id': 'dev.raygun.test.throws',
        'name': 'Test: always fails',
        'description': 'Every search and chart throws an error. Tests calm "Issue with source" pages.',
        'provides': ['music.metadata', 'manga.source'],
        'code': """
const fail = async () => { throw new Error('This test source fails on purpose (HTTP 503 from example.invalid)'); };
raygun.providers.register({ kind: 'music.metadata', id: 'search', name: 'Test: always fails', search: fail });
raygun.providers.register({
  kind: 'manga.source', id: 'manga', name: 'Test: always fails',
  popular: fail, search: fail, details: fail, chapters: fail, pages: fail,
});
""",
    },
    {
        'id': 'dev.raygun.test.slow',
        'name': 'Test: never answers',
        'description': 'Searches wait until the host gives up on them. Tests timeouts.',
        'provides': ['music.metadata'],
        'code': """
raygun.providers.register({
  kind: 'music.metadata', id: 'search', name: 'Test: never answers',
  search: async () => { await raygun.time.sleep(600000); return { tracks: [] }; },
});
""",
    },
    {
        'id': 'dev.raygun.test.garbage',
        'name': 'Test: garbage results',
        'description': 'Answers with missing fields, wrong types, huge text and bad links. Tests that bad data is dropped, not shown.',
        'provides': ['music.metadata', 'music.stream', 'manga.source'],
        'code': """
const junk = [
  { title: '' },
  { title: null, artist: 42 },
  { title: 'x'.repeat(5000), artist: 'y'.repeat(5000), artworkUrl: 'javascript:alert(1)' },
  { title: 'Valid but odd', artist: ['not', 'a', 'string'], durationMs: -5 },
  'not an object',
  null,
  { title: 'Garbage test song', artist: 'Raygun tests', durationMs: 180000, artworkUrl: 'ftp://nope/x.jpg' },
];
raygun.providers.register({
  kind: 'music.metadata', id: 'search', name: 'Test: garbage results',
  search: async () => ({ tracks: junk }),
});
raygun.providers.register({
  kind: 'music.stream', id: 'stream', name: 'Test: garbage streams',
  candidates: async () => ({ candidates: [{ id: 1, title: 'Garbage test song', artist: 'Raygun tests' }, { id: null }, 'junk'] }),
  resolve: async () => ({ url: 'not a url', headers: 7 }),
});
raygun.providers.register({
  kind: 'manga.source', id: 'manga', name: 'Test: garbage manga',
  popular: async () => ({ items: junk, hasNextPage: 'yes' }),
  search: async () => ({ items: [{ id: '', title: '' }, { id: 'ok', title: 'Garbage test comic', coverUrl: 'http://127.0.0.1:1/x.png' }], hasNextPage: false }),
  details: async () => ({ title: 'Garbage test comic', description: 42, genres: 'not a list' }),
  chapters: async () => ({ chapters: [{ id: 'c1', name: 'Chapter with dead pages' }, { id: '', name: '' }] }),
  pages: async () => ({ pages: ['https://example.invalid/1.jpg', { url: 'https://httpbin.org/status/404' }, 7] }),
});
""",
    },
    {
        'id': 'dev.raygun.test.dead-streams',
        'name': 'Test: dead streams',
        'description': 'Offers streams for every song that answer 404 or 403. Tests that the player moves on.',
        'provides': ['music.stream'],
        'code': """
raygun.providers.register({
  kind: 'music.stream', id: 'stream', name: 'Test: dead streams',
  candidates: async (query) => ({ candidates: [
    { id: 'gone', title: query.title, artist: query.artist, durationMs: query.durationMs, quality: '320k',
      source: { kind: 'url', url: 'https://httpbin.org/status/404', headers: {} } },
    { id: 'forbidden', title: query.title, artist: query.artist, durationMs: query.durationMs, quality: '320k',
      source: { kind: 'url', url: 'https://httpbin.org/status/403', headers: {} } },
  ] }),
});
""",
        'net': ['httpbin.org'],
    },
    {
        'id': 'dev.raygun.test.crash-on-load',
        'name': 'Test: crashes while loading',
        'description': 'Throws while it loads. Tests how an extension that never starts is shown.',
        'provides': ['music.metadata'],
        'code': """
throw new Error('This test extension crashes while loading, on purpose');
""",
    },
]


def build_extensions():
    packages = os.path.join(OUT, 'packages')
    os.makedirs(packages, exist_ok=True)
    entries = []
    for ext in EXTENSIONS:
        manifest = {
            'id': ext['id'],
            'name': ext['name'],
            'version': '1.0.0',
            'apiVersion': 1,
            'runtime': 'js',
            'entry': 'dist/index.js',
            'description': ext['description'],
            'author': 'Raygun tests',
            'provides': ext['provides'],
            'permissions': {'net': ext.get('net', ['example.invalid'])},
            'contentRating': 'safe',
        }
        code = f"// {ext['id']} 1.0.0: a Raygun test source that fails on purpose (M46.17).\n\"use strict\";\n" + ext['code'].lstrip()
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, 'w', zipfile.ZIP_DEFLATED) as archive:
            for name, data in [('raygun-extension.json', json.dumps(manifest, indent=2)), ('dist/index.js', code)]:
                info = zipfile.ZipInfo(name, date_time=(2000, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(info, data)
        data = buffer.getvalue()
        file = f"{ext['id']}-1.0.0.zip"
        with open(os.path.join(packages, file), 'wb') as handle:
            handle.write(data)
        entry = {key: value for key, value in manifest.items() if key not in ('runtime', 'entry')}
        entry['permissions'] = {'net': manifest['permissions']['net'], 'webview': False}
        entry['package'] = f'packages/{file}'
        entry['sha256'] = hashlib.sha256(data).hexdigest()
        entry['size'] = len(data)
        entries.append(entry)
    index = {'formatVersion': 1, 'name': 'Raygun test sources (fail on purpose)', 'extensions': entries}
    with open(os.path.join(OUT, 'index.json'), 'w', encoding='utf-8', newline='\n') as handle:
        handle.write(json.dumps(index, indent=2) + '\n')


# ---- A static Stremio addon of legal films and channels ----

DEAD_404 = 'https://download.blender.org/peach/bigbuckbunny_movies/BigBuckBunny_320x180.mp4'
DEAD_403 = 'https://storage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4'

FILMS = [
    {
        'id': 'rgtest-sintel', 'name': 'Sintel (fallback test)', 'year': 2010,
        'poster': 'https://archive.org/services/img/Sintel',
        'description': 'Blender Foundation, CC BY 3.0. Two dead links come first: the player should move on to a working one by itself.',
        'streams': [
            ('Dead link (404)', DEAD_404),
            ('Dead link (403)', DEAD_403),
            ('Sintel 1080p (Blender)', 'https://download.blender.org/durian/movies/Sintel.2010.1080p.mkv'),
            ('Sintel (Internet Archive)', 'https://archive.org/download/Sintel/sintel-2048-stereo.mp4'),
        ],
    },
    {
        'id': 'rgtest-tears', 'name': 'Tears of Steel (HLS and DASH)', 'year': 2012,
        'poster': 'https://archive.org/services/img/tears-of-steel',
        'description': 'Blender Foundation, CC BY 3.0. Adaptive streams: HLS and DASH test servers, then the file.',
        'streams': [
            ('HLS (Unified Streaming)', 'https://demo.unified-streaming.com/k8s/features/stable/video/tears-of-steel/tears-of-steel.ism/.m3u8'),
            ('DASH (Unified Streaming)', 'https://demo.unified-streaming.com/k8s/features/stable/video/tears-of-steel/tears-of-steel.ism/.mpd'),
            ('HLS (Mux)', 'https://test-streams.mux.dev/tos_ismc/main.m3u8'),
            ('720p file (Blender)', 'https://download.blender.org/demo/movies/ToS/tears_of_steel_720p.mov'),
        ],
        'subtitles': [('eng', 'https://download.blender.org/demo/movies/ToS/subtitles/TOS-en.srt')],
    },
    {
        'id': 'rgtest-bbb', 'name': 'Big Buck Bunny', 'year': 2008,
        'poster': 'https://archive.org/services/img/BigBuckBunny_124',
        'description': 'Blender Foundation, CC BY 3.0, from the Internet Archive and test HLS servers.',
        'streams': [
            ('HLS (Mux)', 'https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8'),
            ('DASH (Akamai)', 'https://dash.akamaized.net/akamai/bbb_30fps/bbb_30fps.mpd'),
            ('Internet Archive', 'https://archive.org/download/BigBuckBunny_124/Content/big_buck_bunny_720p_surround.mp4'),
        ],
    },
    {
        'id': 'rgtest-elephants', 'name': "Elephants Dream", 'year': 2006,
        'poster': 'https://archive.org/services/img/ElephantsDream',
        'description': 'Blender Foundation, CC BY 2.5, from the Internet Archive.',
        'streams': [('Internet Archive', 'https://archive.org/download/ElephantsDream/ed_hd.mp4')],
    },
    {
        'id': 'rgtest-broken', 'name': 'Every link broken (test)', 'year': 2026,
        'poster': None,
        'description': 'Every link is dead on purpose: the player should say so calmly, not hang.',
        'streams': [('Dead link (404)', DEAD_404), ('Dead link (403)', DEAD_403), ('Not a host', 'https://example.invalid/film.mp4')],
    },
]

CHANNELS = [
    ('rgtest-dw', 'DW English', [('DW', 'https://dwamdstream102.akamaized.net/hls/live/2015525/dwstream102/master.m3u8')]),
    ('rgtest-nhk', 'NHK World', [('NHK', 'https://media-tyo.hls.nhkworld.jp/hls/w/live/master.m3u8')]),
    ('rgtest-euronews', 'Euronews English', [('Euronews', 'https://cdn-euronews.akamaized.net/live/eds/euronews-en/25002/index.m3u8')]),
    ('rgtest-abc', 'ABC News Live', [('ABC News', 'https://abcnews-streams.akamaized.net/hls/live/2023560/abcnewshudson1/master.m3u8')]),
    ('rgtest-aljazeera', 'Al Jazeera English (first host dead)', [
        ('Web host (dead)', 'https://live-hls-web-aje.getaj.net/AJE/index.m3u8'),
        ('Apps host', 'https://live-hls-apps-aje-fa.getaj.net/AJE/index.m3u8'),
    ]),
]


def write_json(path, value):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w', encoding='utf-8', newline='\n') as handle:
        handle.write(json.dumps(value, indent=2, ensure_ascii=False) + '\n')


def build_stremio():
    base = os.path.join(OUT, 'stremio')
    write_json(os.path.join(base, 'manifest.json'), {
        'id': 'dev.raygun.test-videos',
        'version': '1.0.0',
        'name': 'Raygun test videos',
        'description': 'Legal test films (Blender, Internet Archive) and live news channels, with dead links first to test fallback. For testing Raygun only.',
        'types': ['movie', 'tv'],
        'catalogs': [
            {'type': 'movie', 'id': 'rgtest', 'name': 'Test films'},
            {'type': 'tv', 'id': 'rgtest-live', 'name': 'Test live channels'},
        ],
        'resources': ['catalog', 'meta', 'stream', 'subtitles'],
        'idPrefixes': ['rgtest'],
    })
    metas = []
    for film in FILMS:
        meta = {
            'id': film['id'], 'type': 'movie', 'name': film['name'], 'year': film['year'],
            'poster': film['poster'], 'description': film['description'], 'genres': ['Animation', 'Test'],
        }
        metas.append(meta)
        write_json(os.path.join(base, 'meta', 'movie', f"{film['id']}.json"), {'meta': meta})
        write_json(os.path.join(base, 'stream', 'movie', f"{film['id']}.json"), {
            'streams': [
                {'name': 'Raygun test', 'title': title, 'url': url, 'behaviorHints': {'bingeGroup': 'raygun-test'}}
                for title, url in film['streams']
            ],
        })
        write_json(os.path.join(base, 'subtitles', 'movie', f"{film['id']}.json"), {
            'subtitles': [
                {'id': f"{film['id']}-{lang}", 'lang': lang, 'url': url}
                for lang, url in film.get('subtitles', [])
            ],
        })
    write_json(os.path.join(base, 'catalog', 'movie', 'rgtest.json'), {'metas': metas})
    channels = []
    for channel_id, name, streams in CHANNELS:
        meta = {'id': channel_id, 'type': 'tv', 'name': name, 'description': f'{name}, live from the broadcaster.', 'genres': ['News']}
        channels.append(meta)
        write_json(os.path.join(base, 'meta', 'tv', f'{channel_id}.json'), {'meta': meta})
        write_json(os.path.join(base, 'stream', 'tv', f'{channel_id}.json'), {
            'streams': [{'name': 'Live', 'title': title, 'url': url} for title, url in streams],
        })
    write_json(os.path.join(base, 'catalog', 'tv', 'rgtest-live.json'), {'metas': channels})


if __name__ == '__main__':
    build_extensions()
    build_stremio()
    print('built', OUT, file=sys.stderr)
