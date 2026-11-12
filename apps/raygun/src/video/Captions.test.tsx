import { act, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const MOVIE = {
  id: 'stremio:movie:tt1',
  url: 'http://127.0.0.1:1/media/t/https/cdn.example/movie.mp4',
  format: 'mp4',
  title: 'The Movie',
  subtitle: null,
  subtitles: [],
  startMs: 0,
  seekStepMs: 10_000,
  group: 'stremio:movie:tt1',
  titleState: null,
  chapters: [],
};

const track = (lang: string, label: string) => ({
  url: `http://127.0.0.1:1/subtitle/${label}`,
  lang,
  label,
  format: 'srt',
});

const SRT = '1\n00:00:01,000 --> 00:00:03,000\nHello there\n';

const open = (json: Record<string, unknown>) =>
  emitEvent('videoPlayerEvent', { kind: 'open', json: JSON.stringify(json) });
const update = (json: Record<string, unknown>) =>
  emitEvent('videoPlayerEvent', { kind: 'update', json: JSON.stringify(json) });

const reports = () =>
  commands.videoPlayerReport.mock.calls.map(
    ([json]) => JSON.parse(json as string) as Record<string, unknown>,
  );

/** Opens the player on the film with `extra`; the video knows its length. */
const play = async (extra: Record<string, unknown> = {}) => {
  await AppShellWrapper.mount('/');
  open({ ...MOVIE, ...extra });
  const player = await screen.findByRole('dialog', { name: 'Video player' });
  const video = await within(player).findByTestId('desktop-video');
  Object.defineProperty(video, 'duration', { value: 600, configurable: true });
  fireEvent(video, new Event('loadedmetadata'));
  return { player, video };
};

const openPanel = async (player: HTMLElement) => {
  await userEvent.setup().click(within(player).getByRole('button', { name: 'Subtitles' }));
  return within(player).findByRole('region', { name: 'Subtitles' });
};

/** Moves the picture to `seconds`, as playing would. */
const playAt = (video: HTMLElement, seconds: number) => {
  (video as HTMLVideoElement).currentTime = seconds;
  fireEvent(video, new Event('timeupdate'));
};

/** English subtitles on (the audio is Japanese). */
const WITH_ENGLISH = {
  subtitles: [track('en', 'English')],
  spokenLanguage: 'ja',
  subtitleMode: 'always',
};

describe('captions in the desktop player', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () => new Response(SRT));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('lists your languages first, every track in one together', async () => {
    const { player } = await play({
      subtitles: [
        track('spa', 'Español'),
        track('en', 'English'),
        track('fre', 'Français'),
        track('eng', 'English SDH'),
      ],
      subtitleMode: 'never',
    });
    const panel = await openPanel(player);
    const names = within(panel)
      .getAllByRole('menuitemradio')
      .map((item) => item.querySelector('.truncate')?.textContent);
    // The device's language (English) first, then the rest as the video lists them.
    expect(names.slice(0, 5)).toEqual(['Off', 'English', 'English SDH', 'Español', 'Français']);
  });

  it('names tracks in one language apart, where each comes from under it', async () => {
    const { player } = await play({
      subtitles: [
        { ...track('en', 'English'), source: 'OpenSubtitles v3' },
        { ...track('eng', 'eng'), source: 'OpenSubtitles v3' },
        { ...track('en', 'English'), url: 'http://127.0.0.1:1/subtitle/other', source: 'SubDL' },
        track('en', 'English Forced'),
      ],
      subtitleMode: 'never',
    });
    const panel = await openPanel(player);
    const rows = within(panel)
      .getAllByRole('menuitemradio')
      .map((item) => Array.from(item.querySelectorAll('.truncate'), (line) => line.textContent));
    // The tracks, then Find subtitles' own row.
    expect(rows.slice(0, 5)).toEqual([
      ['Off'],
      ['English', 'OpenSubtitles v3'],
      ['English 2', 'OpenSubtitles v3'],
      ['English 3', 'SubDL'],
      // Its own name tells it apart; a file without an addon still says it's a file.
      ['English Forced', 'Subtitle file'],
    ]);
  });

  it('opens the style from a card of its own, with a sample, not a row like the tracks', async () => {
    const user = userEvent.setup();
    const { player } = await play(WITH_ENGLISH);
    const panel = await openPanel(player);
    const card = within(panel).getByRole('button', { name: /^Style/ });
    expect(card).toHaveTextContent('Default · 100%');
    expect(within(card).getByTestId('subtitle-style-sample')).toHaveTextContent('Aa');
    expect(within(panel).queryByRole('menuitemradio', { name: /Style/ })).toBeNull();
    await user.click(card);
    expect(await within(player).findByTestId('video-settings-panel')).toBeInTheDocument();
    expect(within(player).getByRole('button', { name: /^Font/ })).toBeInTheDocument();
  });

  it('draws both subtitles in a font picked while they show', async () => {
    const user = userEvent.setup();
    const { player, video } = await play(WITH_ENGLISH);
    // The same file second, at the top.
    const panel = await openPanel(player);
    await user.click(within(panel).getByRole('tab', { name: 'Second subtitles' }));
    await user.click(within(panel).getByRole('menuitemradio', { name: /^English/ }));
    playAt(video, 1.5);
    await vi.waitFor(() => expect(within(player).getAllByText('Hello there')).toHaveLength(2));

    update({ type: 'prefs', prefs: { subtitleStyle: { font: 'casual' } } });
    await vi.waitFor(() => {
      for (const line of within(player).getAllByText('Hello there')) {
        expect(line.style.fontFamily).toMatch(/^casual, "Comic Sans MS"/);
      }
    });
    expect(within(player).getByTestId('second-subtitle-lines')).toBeInTheDocument();
  });

  it('adds subtitles with buttons of their own, apart from the list', async () => {
    const user = userEvent.setup();
    const { player } = await play();
    const panel = await openPanel(player);
    // The tools are buttons, not choices in the list; AI subtitles aren't a choice before
    // there are any.
    expect(within(panel).queryByRole('menuitemradio', { name: /Transcribe|Translate/ })).toBeNull();
    expect(within(panel).queryByRole('menuitemradio', { name: /AI subtitles/ })).toBeNull();
    await user.click(within(panel).getByRole('button', { name: 'Transcribe with AI' }));
    expect(within(panel).getByRole('heading', { name: 'Transcribe with AI' })).toBeVisible();
    await user.click(within(panel).getByRole('button', { name: 'All subtitles' }));
    await user.click(within(panel).getByRole('button', { name: 'Translate subtitles' }));
    expect(within(panel).getByRole('heading', { name: 'Translate subtitles' })).toBeVisible();
    await user.click(within(panel).getByRole('button', { name: 'All subtitles' }));

    // A file added here shows at once.
    await user.upload(
      within(panel).getByLabelText('Add a subtitle file'),
      new File([SRT], 'film.srt', { type: 'text/plain' }),
    );
    expect(await within(panel).findByRole('menuitemradio', { name: /film\.srt/ })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });

  it('types the delay in seconds, earlier or later, up to ten minutes', async () => {
    const user = userEvent.setup();
    const { player } = await play();
    const panel = await openPanel(player);
    const delay = within(panel).getByRole('textbox', { name: 'Subtitle delay in seconds' });
    await user.clear(delay);
    await user.type(delay, '-12,5{Enter}');
    expect(reports()).toContainEqual(
      expect.objectContaining({ action: 'state', change: { subtitleDelayMs: -12_500 } }),
    );
    expect(delay).toHaveValue('-12.50');
    await user.clear(delay);
    await user.type(delay, '900{Enter}');
    expect(reports()).toContainEqual(
      expect.objectContaining({ action: 'state', change: { subtitleDelayMs: 600_000 } }),
    );
  });

  it('sizes subtitles by the picture as it shows, not the window', async () => {
    const { player, video } = await play(WITH_ENGLISH);
    // A tall window with a wide film: the picture is 1000×417 in a 1000×2000 player.
    for (const [name, value] of [
      ['clientWidth', 1000],
      ['clientHeight', 2000],
      ['videoWidth', 1920],
      ['videoHeight', 800],
    ] as const) {
      Object.defineProperty(video, name, { value, configurable: true });
    }
    fireEvent(video, new Event('resize'));
    playAt(video, 1.5);
    const line = await within(player).findByText('Hello there');
    expect(Number.parseFloat(line.style.fontSize)).toBeCloseTo((1000 / 2.4) * 0.046, 1);
    // At the picture's foot (8 % up it), not the window's.
    const lines = within(player).getByTestId('subtitle-lines');
    expect(Number.parseFloat(lines.style.bottom)).toBeCloseTo(791.67 + 33.33, 0);
  });

  it('shows a style changed in Settings on the lines at once', async () => {
    const { player, video } = await play(WITH_ENGLISH);
    playAt(video, 1.5);
    expect(await within(player).findByText('Hello there')).toHaveStyle({
      color: 'rgb(255, 255, 255)',
    });
    update({ type: 'prefs', prefs: { subtitleStyle: { textColor: 0xffffeb3b } } });
    await vi.waitFor(() =>
      expect(within(player).getByText('Hello there')).toHaveStyle({ color: 'rgb(255, 235, 59)' }),
    );
  });

  it('waits 5 s before showing the ring for a seek, not for a new video', async () => {
    const { player, video } = await play();
    // Before its first frame the ring shows at once.
    fireEvent(video, new Event('waiting'));
    expect(within(player).getByText('Loading')).toBeInTheDocument();
    fireEvent(video, new Event('playing'));
    expect(within(player).queryByText('Loading')).toBeNull();

    vi.useFakeTimers({ shouldAdvanceTime: true });
    fireEvent(video, new Event('seeking'));
    fireEvent(video, new Event('waiting'));
    expect(within(player).queryByText('Loading')).toBeNull();
    await act(() => vi.advanceTimersByTimeAsync(4_800));
    expect(within(player).queryByText('Loading')).toBeNull();
    await act(() => vi.advanceTimersByTimeAsync(400));
    expect(within(player).getByText('Loading')).toBeInTheDocument();
    fireEvent(video, new Event('canplay'));
    expect(within(player).queryByText('Loading')).toBeNull();
  });
});
