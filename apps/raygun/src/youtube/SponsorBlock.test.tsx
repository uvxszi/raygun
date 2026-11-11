import type { SkipSegment } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { SeekBar } from '../player/SeekBar';
import { usePlaybackSync } from '../player/usePlayback';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, IDLE_STATE, bindingsMock, emitEvent } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { useTrailerStore } from '../video/trailerStore';
import { PartSkipper } from './skipParts';
import { SPONSORBLOCK_DEFAULTS } from './youtubeDefaults';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const part = (
  id: string,
  startMs: number,
  endMs: number,
  overrides: Partial<SkipSegment> = {},
): SkipSegment => ({ id, category: 'sponsor', startMs, endMs, action: 'skip', ...overrides });

/** Saves echo back the stored settings with the change applied, as Rust does. */
const storeSettings = () => {
  let stored = { ...DEFAULT_SETTINGS };
  commands.settingsGet.mockImplementation(() => Promise.resolve(ok(stored)));
  commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) => {
    stored = { ...stored, [change.key]: change.value };
    return Promise.resolve(ok(stored));
  });
};

const lastYouTube = () =>
  commands.settingsUpdate.mock.calls
    .map(([change]) => change as { key: string; value: typeof DEFAULT_SETTINGS.youtube })
    .filter((change) => change.key === 'youtube')
    .at(-1)?.value;

describe('SponsorBlock and YouTube settings', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('sets what each kind of part does, the downloads and the audio track', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/music');
    const group = await screen.findByRole('region', { name: 'SponsorBlock' });
    expect(within(group).getByText('sponsor.ajay.app', { exact: false })).toBeInTheDocument();

    await user.click(within(group).getByRole('button', { name: /^Sponsors/ }));
    const sheet = await screen.findByRole('dialog');
    await user.click(within(sheet).getByRole('radio', { name: 'Show on the seek bar' }));
    await waitFor(() => expect(lastYouTube()?.sponsorBlock?.sponsor).toBe('show'));
    // The other kinds stay as they were.
    expect(lastYouTube()?.sponsorBlock?.musicOfftopic).toBe('skip');

    await user.click(
      within(group).getByRole('switch', { name: 'Leave non-music parts out of downloads' }),
    );
    await waitFor(() => expect(lastYouTube()?.sponsorBlock?.trimDownloads).toBe(true));

    await user.click(screen.getByRole('switch', { name: 'Original audio' }));
    await waitFor(() => expect(lastYouTube()?.originalAudio).toBe(false));

    // Off: the kinds can't be changed.
    await user.click(within(group).getByRole('switch', { name: 'Use SponsorBlock' }));
    await waitFor(() => expect(lastYouTube()?.sponsorBlock?.enabled).toBe(false));
    expect(within(group).getByRole('button', { name: /^Intros/ })).toBeDisabled();
  });

  it('offers H.264 among the video player’s advanced settings', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/video/advanced');
    await user.click(await screen.findByRole('button', { name: /^Video codec/ }));
    const sheet = await screen.findByRole('dialog');
    await user.click(within(sheet).getByRole('radio', { name: /^Prefer H\.264/ }));
    await waitFor(() => {
      const saved = commands.settingsUpdate.mock.calls
        .map(([change]) => change as { key: string; value: { videoCodec?: string } })
        .filter((change) => change.key === 'videoPlayer')
        .at(-1)?.value;
      expect(saved?.videoCodec).toBe('h264');
    });
  });
});

describe('Skipping parts of songs', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('says what was skipped and takes it back with Undo', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    emitEvent('skippedSegmentEvent', {
      itemId: 'id-a',
      segment: part('outro-1', 180_000, 212_000, { category: 'musicOfftopic' }),
    });
    const toast = await screen.findByTestId('toast');
    expect(toast).toHaveTextContent('Skipped: Non-music parts of music videos');
    await user.click(within(toast).getByRole('button', { name: 'Undo' }));
    expect(commands.playbackUndoSkip).toHaveBeenCalledWith('outro-1');
  });

  it('marks the song’s parts on its seek bar, only for the song they are of', async () => {
    AppShellWrapper.seedCore();
    commands.playbackGetState.mockResolvedValue({
      ...IDLE_STATE,
      status: 'playing',
      currentItemId: 'id-a',
      positionMs: 1_000,
      durationMs: 200_000,
    });
    commands.playbackSkipSegments.mockResolvedValue({
      itemId: 'id-a',
      videoId: 'kJQP7kiw5Fk',
      segments: [
        part('intro', 0, 20_000, { category: 'musicOfftopic' }),
        part('promo', 100_000, 110_000, { category: 'selfpromo', action: 'show' }),
      ],
    });
    const Bar = () => {
      usePlaybackSync();
      return <SeekBar variant="sheet" />;
    };
    renderWithProviders(<Bar />);
    const marks = await screen.findByTestId('skip-marks');
    const drawn = marks.querySelectorAll('[data-category]');
    expect(drawn).toHaveLength(2);
    expect(drawn[0]).toHaveStyle({ left: '0%', width: '10%' });
    expect(drawn[1]).toHaveAttribute('data-category', 'selfpromo');

    // Another song plays: its own parts aren't known yet.
    emitEvent('playbackStateEvent', {
      ...IDLE_STATE,
      status: 'playing',
      currentItemId: 'id-b',
      positionMs: 0,
      durationMs: 200_000,
    });
    await waitFor(() => expect(screen.queryByTestId('skip-marks')).not.toBeInTheDocument());
  });
});

describe('YouTube’s player in Raygun’s window', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('skips the parts it reaches, marks them on YouTube’s seek bar, and plays one on Undo', async () => {
    const user = userEvent.setup();
    commands.sponsorblockSegments.mockResolvedValue(
      ok([part('sponsor-1', 30_000, 90_000), part('intro-1', 0, 8_000, { action: 'show' })]),
    );
    await AppShellWrapper.mount('/');
    useTrailerStore.getState().open({
      videoId: 'aqz-KE-bpKQ',
      title: 'A film',
      subtitle: null,
      embedUrl: 'http://localhost:4100/embed/youtube?controls=1&audio=original&codec=auto',
    });
    const frame = (await screen.findByTitle('Trailer player')) as HTMLIFrameElement;
    const host = frame.contentWindow!;
    const posted = vi.spyOn(host, 'postMessage').mockImplementation(() => {});
    const fromPage = (data: Record<string, unknown>) =>
      window.dispatchEvent(
        new MessageEvent('message', { data: { raygunEmbed: true, ...data }, source: host }),
      );
    const sent = () => posted.mock.calls.map(([message]) => message as Record<string, unknown>);

    fromPage({ type: 'ready' });
    expect(sent()[0]).toMatchObject({ raygunCommand: true, type: 'load', videoId: 'aqz-KE-bpKQ' });
    await waitFor(() =>
      expect(sent().find((message) => message.type === 'segments')).toMatchObject({
        videoId: 'aqz-KE-bpKQ',
        segments: [
          { startMs: 30_000, endMs: 90_000, category: 'sponsor' },
          { startMs: 0, endMs: 8_000, category: 'sponsor' },
        ],
      }),
    );

    // A part only shown plays; one to skip is jumped over.
    fromPage({ type: 'progress', positionMs: 2_000, durationMs: 600_000 });
    expect(sent().some((message) => message.type === 'seek')).toBe(false);
    fromPage({ type: 'progress', positionMs: 31_000, durationMs: 600_000 });
    expect(sent().filter((message) => message.type === 'seek')).toEqual([
      { raygunCommand: true, type: 'seek', positionMs: 90_000 },
    ]);
    const toast = await screen.findByTestId('toast');
    expect(toast).toHaveTextContent('Skipped: Sponsors');
    await user.click(within(toast).getByRole('button', { name: 'Undo' }));
    expect(
      sent()
        .filter((message) => message.type === 'seek')
        .at(-1),
    ).toEqual({
      raygunCommand: true,
      type: 'seek',
      positionMs: 30_000,
    });
    // Taken back: it plays through this time.
    fromPage({ type: 'progress', positionMs: 30_500, durationMs: 600_000 });
    expect(sent().filter((message) => message.type === 'seek')).toHaveLength(2);
  });
});

describe('PartSkipper', () => {
  it('passes parts that follow on in one jump, never past the end, and lets late reports be', () => {
    const skipper = new PartSkipper();
    const parts = [
      part('a', 0, 12_000),
      part('b', 12_500, 30_000),
      part('end', 170_000, 190_000),
      part('short', 50_000, 50_400),
    ];
    expect(skipper.due(parts, 500, 180_000, 0)?.toMs).toBe(30_000);
    // Still from before the jump.
    expect(skipper.due(parts, 900, 180_000, 500)).toBeNull();
    expect(skipper.due(parts, 50_100, 180_000, 10_000)).toBeNull();
    expect(skipper.due(parts, 171_000, 180_000, 20_000)?.toMs).toBe(180_000);
    expect(SPONSORBLOCK_DEFAULTS.musicOfftopic).toBe('skip');
  });
});
