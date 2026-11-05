import type { SubtitleTranslation } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { coveredAhead, toCaptions } from './subtitleProgress';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const MOVIE = {
  id: 'stremio:movie:tt1',
  url: 'http://127.0.0.1:1/media/t/https/cdn.example/movie.mp4',
  format: 'mp4',
  title: 'The Movie',
  subtitle: null,
  subtitles: [],
  startMs: 61_000,
  seekStepMs: 10_000,
  group: 'stremio:movie:tt1',
  titleState: null,
  chapters: [],
};

const ENGLISH = {
  url: 'http://127.0.0.1:1/subtitle/en',
  lang: 'en',
  label: 'English',
  format: 'srt',
};

const SRT =
  '1\n00:00:01,000 --> 00:00:03,000\nHello there\n\n2\n00:00:04,000 --> 00:00:05,000\nBye\n';

const translation = (change: Partial<SubtitleTranslation> = {}): SubtitleTranslation => ({
  key: MOVIE.id,
  id: '0123456789abcdef.de',
  language: 'de',
  translator: 'ai:claude',
  by: 'Claude',
  sourceLabel: 'English',
  sourceLanguage: 'en',
  cues: [{ startMs: 1_000, endMs: 3_000, text: 'Hallo' }],
  done: 1,
  total: 2,
  aheadMs: 30_000,
  // The viewer was at 1:01: lines are ready until 1:31.
  readyUntilMs: 91_000,
  complete: false,
  running: true,
  downloading: null,
  error: null,
  downloadBytes: 0,
  ...change,
});

/** Opens the player on a film with `subtitles`, at `timeMs`, and its Subtitles panel. */
const openPanel = async (subtitles: unknown[]) => {
  await AppShellWrapper.mount('/');
  emitEvent('videoPlayerEvent', {
    kind: 'open',
    json: JSON.stringify({
      ...MOVIE,
      subtitles,
      spokenLanguage: 'ja',
      subtitleMode: 'always',
    }),
  });
  const player = await screen.findByRole('dialog', { name: 'Video player' });
  const video = await within(player).findByTestId('desktop-video');
  Object.defineProperty(video, 'duration', { value: 600, configurable: true });
  fireEvent(video, new Event('loadedmetadata'));
  await userEvent.setup().click(within(player).getByRole('button', { name: 'Subtitles' }));
  const panel = await within(player).findByRole('region', { name: 'Subtitles' });
  return { player, video, panel };
};

/** Moves the picture to `seconds`, as playing would. */
const playAt = (video: HTMLElement, seconds: number) => {
  (video as HTMLVideoElement).currentTime = seconds;
  fireEvent(video, new Event('timeupdate'));
};

describe('AI subtitles and translations in the desktop player', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () => new Response(SRT));
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('downloads the speech model in place when transcription isn’t set up', async () => {
    const user = userEvent.setup();
    commands.aiModels.mockResolvedValue([
      {
        id: 'whisper-tiny',
        name: 'Speech to text (Whisper tiny)',
        kind: 'speech',
        description: '',
        license: 'Apache-2.0',
        licenseUrl: '',
        sourceUrl: '',
        sizeBytes: 153_544_121,
        installed: false,
        downloading: null,
      },
    ]);
    const { panel } = await openPanel([]);
    expect(within(panel).getByRole('heading', { name: 'Transcribe with AI' })).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Set up in Settings → AI' })).toBeVisible();

    await user.click(
      await within(panel).findByRole('button', { name: 'Download the speech model (146 MB)' }),
    );
    expect(commands.aiModelDownload).toHaveBeenCalledWith('whisper-tiny');
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenCalledWith({
        key: 'transcription',
        value: expect.objectContaining({ engine: 'local', localModel: 'whisper-tiny' }),
      }),
    );
  });

  it('says how far ahead of the viewer transcribing is', async () => {
    commands.aiModels.mockResolvedValue([
      {
        id: 'whisper-tiny',
        name: 'Speech to text (Whisper tiny)',
        kind: 'speech',
        description: '',
        license: 'Apache-2.0',
        licenseUrl: '',
        sourceUrl: '',
        sizeBytes: 1,
        installed: true,
        downloading: null,
      },
    ]);
    commands.captions.mockImplementation(async (episodeId: string) => ({
      status: 'ok',
      data: {
        episodeId,
        cues: [{ startMs: 60_000, endMs: 62_000, text: 'こんにちは' }],
        source: 'local',
        covered: [
          [30_000, 60_000],
          [60_000, 90_000],
        ],
        complete: false,
        running: true,
        preparing: false,
        error: null,
        canTranscribe: true,
        engine: 'local',
        hasFeed: false,
        queued: null,
        durationMs: 600_000,
      },
    }));
    const { panel } = await openPanel([]);
    // The viewer is at 1:01; what's heard reaches 1:30.
    expect(await within(panel).findByRole('status')).toHaveTextContent('Transcribing… 0:29 ahead');
    expect(within(panel).getByRole('button', { name: 'Stop' })).toBeVisible();
  });

  it('translates the subtitles shown, ahead of the viewer, into a track of its own', async () => {
    const user = userEvent.setup();
    commands.videoSubtitlesTranslators.mockResolvedValue({
      status: 'ok',
      data: [
        {
          id: 'device',
          kind: null,
          name: '',
          models: [],
          downloadBytes: 0,
          supported: false,
        },
        {
          id: 'ai:claude',
          kind: 'claude',
          name: 'Claude',
          models: [],
          downloadBytes: 0,
          supported: true,
        },
      ],
    });
    commands.videoSubtitlesTranslate.mockResolvedValue({ status: 'ok', data: translation() });
    const { player, video, panel } = await openPanel([ENGLISH]);
    await within(panel).findByText('From: English');
    await user.selectOptions(within(panel).getByRole('combobox', { name: 'Into' }), 'de');
    // This device has no model for the language; Claude has.
    expect(within(panel).getByRole('menuitemradio', { name: /This device/ })).toBeDisabled();
    commands.videoSubtitlesTranslations.mockResolvedValue({ status: 'ok', data: [translation()] });
    await user.click(within(panel).getByRole('menuitemradio', { name: 'With Claude' }));

    expect(commands.videoSubtitlesTranslate).toHaveBeenCalledWith({
      key: MOVIE.id,
      source: {
        kind: 'cues',
        label: 'English',
        language: 'en',
        cues: [
          { startMs: 1_000, endMs: 3_000, text: 'Hello there' },
          { startMs: 4_000, endMs: 5_000, text: 'Bye' },
        ],
      },
      language: 'de',
      translator: 'ai:claude',
      fromMs: 61_000,
      download: false,
    });
    // The translation shows at once, and as more lines come.
    const track = await within(panel).findByRole('menuitemradio', {
      name: /Deutsch \(translated\)/,
    });
    expect(track).toHaveTextContent('Translating… 0:30 ahead');
    await vi.waitFor(() => expect(track).toHaveAttribute('aria-checked', 'true'));
    playAt(video, 1.5);
    expect(within(player).getByTestId('subtitle-lines')).toHaveTextContent('Hallo');

    // The original stays a choice for the second subtitles, at the top.
    await user.selectOptions(
      within(panel).getByRole('combobox', { name: 'Second subtitles' }),
      'file:0',
    );
    playAt(video, 2);
    expect(within(player).getByTestId('second-subtitle-lines')).toHaveTextContent('Hello there');

    // Stopping keeps what's translated.
    await user.click(within(panel).getByRole('button', { name: 'Stop' }));
    expect(commands.videoSubtitlesStop).toHaveBeenCalledWith(MOVIE.id, '0123456789abcdef.de');
  });

  it('asks before this device downloads its translation models', async () => {
    const user = userEvent.setup();
    commands.videoSubtitlesTranslators.mockResolvedValue({
      status: 'ok',
      data: [
        {
          id: 'device',
          kind: null,
          name: '',
          models: ['opus-mt-en-de'],
          downloadBytes: 299_946_724,
          supported: true,
        },
      ],
    });
    commands.videoSubtitlesTranslate.mockResolvedValueOnce({
      status: 'ok',
      data: translation({ running: false, cues: [], done: 0, downloadBytes: 299_946_724 }),
    });
    commands.videoSubtitlesTranslate.mockResolvedValueOnce({
      status: 'ok',
      data: translation({ translator: 'device', downloading: 0 }),
    });
    const { panel } = await openPanel([ENGLISH]);
    await within(panel).findByText('From: English');
    await user.selectOptions(within(panel).getByRole('combobox', { name: 'Into' }), 'de');
    await user.click(await within(panel).findByRole('menuitemradio', { name: /This device/ }));
    expect(
      await within(panel).findByText(
        'Translating on this device downloads 286 MB of models first, once.',
      ),
    ).toBeVisible();
    await user.click(within(panel).getByRole('button', { name: 'Download and translate' }));
    expect(commands.videoSubtitlesTranslate).toHaveBeenLastCalledWith(
      expect.objectContaining({ translator: 'device', language: 'de', download: true }),
    );
  });

  it('needs subtitles that can be read as text', async () => {
    const { panel } = await openPanel([]);
    expect(within(panel).getByText('Turn on subtitles above to translate them.')).toBeVisible();
    expect(commands.videoSubtitlesTranslate).not.toHaveBeenCalled();
  });
});

describe('subtitle progress', () => {
  it('reaches over stretches heard one after another', () => {
    const covered: [number, number][] = [
      [0, 30_000],
      [30_000, 60_000],
      [90_000, 120_000],
    ];
    expect(coveredAhead(covered, 10_000)).toBe(50_000);
    expect(coveredAhead(covered, 60_000)).toBe(0);
    expect(coveredAhead(covered, 95_000)).toBe(25_000);
    expect(toCaptions([{ startMs: 1.4, endMs: 2_000, lines: ['- Hi.', '- Hello.'] }])).toEqual([
      { startMs: 1, endMs: 2_000, text: '- Hi.\n- Hello.' },
    ]);
  });
});
