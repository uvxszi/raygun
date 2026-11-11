import script from '../../../../crates/raygun-app/src/youtube_embed/player.js?raw';

/**
 * Raygun's player script inside YouTube's frame, run against a stand-in for the frame: its
 * window and address, the player element and its seek bar. The script itself is Rust's (an
 * initialization script of every frame); this checks what it does with what the player gives it.
 */

const HOST = 'http://localhost:4100';
const TOP = 'http://tauri.localhost';

type Info = { supported: boolean; smooth: boolean; powerEfficient: boolean };

const frame = ({
  search = '?raygun_audio=original&raygun_codec=h264',
  origins = [HOST, TOP],
  origin = 'https://www.youtube.com',
  info = { supported: true, smooth: true, powerEfficient: true } as Info,
} = {}) => {
  document.body.innerHTML = '';
  const parent = {};
  const ticks: (() => void)[] = [];
  let onMessage: ((event: unknown) => void) | null = null;
  class Media {
    canPlayType(type: string) {
      return type ? 'probably' : '';
    }
  }
  const mediaSource = { isTypeSupported: (type: string) => Boolean(type) };
  const decodingInfo = vi.fn((config: { video: { contentType: string } }) =>
    Promise.resolve(
      config.video.contentType.includes('avc1') ? { ...info, powerEfficient: true } : info,
    ),
  );
  const navigator = { mediaCapabilities: { decodingInfo } };
  const window = {
    top: {},
    parent,
    HTMLMediaElement: Media,
    MediaSource: mediaSource,
    navigator,
    getComputedStyle: () => ({ position: 'static' }),
    addEventListener: (type: string, listener: (event: unknown) => void) => {
      if (type === 'message') onMessage = listener;
    },
  } as Record<string, unknown>;
  new Function('window', 'location', 'document', 'setInterval', 'setTimeout', script)(
    window,
    { origin, ancestorOrigins: origins, search },
    document,
    (tick: () => void) => ticks.push(tick),
    (tick: () => void) => tick(),
  );
  const ask = (data: object, from: unknown = parent, at = HOST) =>
    onMessage?.({ source: from, origin: at, data: { raygunPlayer: true, ...data } });
  const tick = () => ticks.forEach((run) => run());
  return { window, ask, tick, media: new Media(), mediaSource, decodingInfo, navigator };
};

/** YouTube's player element, with its seek bar. */
const player = (tracks: object[], current: object) => {
  const element = document.createElement('div');
  element.id = 'movie_player';
  const bar = document.createElement('div');
  bar.className = 'ytp-progress-list';
  Object.defineProperty(bar, 'offsetWidth', { value: 500 });
  element.appendChild(bar);
  document.body.appendChild(element);
  const methods = {
    getVideoData: () => ({ video_id: 'kJQP7kiw5Fk' }),
    getDuration: () => 200,
    getAvailableAudioTracks: () => tracks,
    getAudioTrack: () => current,
    setAudioTrack: vi.fn(),
  };
  Object.assign(element, methods);
  return { bar, ...methods };
};

describe('the YouTube player script', () => {
  it('stays idle outside Raygun’s own players', () => {
    for (const [origins, origin] of [
      [[HOST, TOP], 'https://example.com'],
      [['https://evil.example', TOP], 'https://www.youtube.com'],
      [[HOST, 'https://evil.example'], 'https://www.youtube.com'],
    ] as const) {
      const { media, mediaSource } = frame({ origins: [...origins], origin });
      expect(media.canPlayType('video/webm; codecs="vp9"')).toBe('probably');
      expect(mediaSource.isTypeSupported('video/webm; codecs="vp9"')).toBe(true);
    }
  });

  it('keeps YouTube’s player to H.264 when asked', async () => {
    const { media, mediaSource, navigator } = frame();
    expect(media.canPlayType('video/webm; codecs="vp9"')).toBe('');
    expect(mediaSource.isTypeSupported('video/mp4; codecs="av01.0.05M.08"')).toBe(false);
    expect(mediaSource.isTypeSupported('video/mp4; codecs="avc1.4d401f"')).toBe(true);
    expect(media.canPlayType('audio/mp4; codecs="mp4a.40.2"')).toBe('probably');
    const vp9 = await navigator.mediaCapabilities.decodingInfo({
      video: { contentType: 'video/webm; codecs="vp09.00.40.08"' },
    });
    expect(vp9).toMatchObject({ supported: false });
  });

  it('refuses only codecs without a power-efficient decoder on auto', async () => {
    const { mediaSource } = frame({
      search: '?raygun_codec=auto',
      info: { supported: true, smooth: false, powerEfficient: false },
    });
    // Until the probes answer, YouTube's player decides.
    expect(mediaSource.isTypeSupported('video/webm; codecs="vp9"')).toBe(true);
    await vi.waitFor(() =>
      expect(mediaSource.isTypeSupported('video/webm; codecs="vp9"')).toBe(false),
    );
    expect(mediaSource.isTypeSupported('video/mp4; codecs="av01.0.05M.08"')).toBe(false);
    expect(mediaSource.isTypeSupported('video/mp4; codecs="avc1.4d401f"')).toBe(true);
    // Opus in WebM is sound: it stays.
    expect(mediaSource.isTypeSupported('audio/webm; codecs="opus"')).toBe(true);
  });

  it('plays a dubbed video’s original audio track', () => {
    const { tick } = frame();
    const dubbed = { id: 'en.10', name: 'English (United States) (auto-dubbed)', isDefault: true };
    // YouTube's own field names change: the details may sit on an object the track holds.
    const original = { xy: { id: 'es.4', name: 'Spanish (Spain) original', isDefault: false } };
    const fake = player([{ xy: dubbed }, original], { xy: dubbed });
    tick();
    expect(fake.setAudioTrack).toHaveBeenCalledWith(original);
    // Once per video.
    tick();
    expect(fake.setAudioTrack).toHaveBeenCalledTimes(1);
  });

  it('leaves the track alone when not asked, or with one track', () => {
    const { tick } = frame({ search: '?raygun_audio=default' });
    const fake = player(
      [
        { id: 'en', name: 'English' },
        { id: 'es', name: 'Spanish original' },
      ],
      { id: 'en', name: 'English' },
    );
    tick();
    expect(fake.setAudioTrack).not.toHaveBeenCalled();
  });

  it('draws the parts its host page passes on YouTube’s seek bar', () => {
    const { ask, tick } = frame();
    const fake = player([], {});
    ask({
      type: 'segments',
      videoId: 'kJQP7kiw5Fk',
      segments: [
        { startMs: 0, endMs: 20_000, category: 'musicOfftopic' },
        { startMs: 100_000, endMs: 120_000, category: 'sponsor' },
      ],
    });
    tick();
    const pieces = fake.bar.querySelectorAll('._raygun_sb_segments > div');
    expect(pieces).toHaveLength(2);
    expect((pieces[0] as HTMLElement).style.width).toBe('50px');
    expect((pieces[1] as HTMLElement).style.left).toBe('250px');
    // Another video: they go.
    fake.getVideoData = () => ({ video_id: 'other123456' });
    Object.assign(document.getElementById('movie_player')!, { getVideoData: fake.getVideoData });
    tick();
    expect(fake.bar.querySelector('._raygun_sb_segments')).toBeNull();
    // Only the host page may ask.
    ask(
      {
        type: 'segments',
        videoId: 'other123456',
        segments: [{ startMs: 0, endMs: 1, category: 'sponsor' }],
      },
      {},
    );
    tick();
    expect(fake.bar.querySelector('._raygun_sb_segments')).toBeNull();
  });
});
