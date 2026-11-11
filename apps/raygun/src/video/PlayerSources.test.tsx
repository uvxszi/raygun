import { act, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

/** The JSON reports the player sent, parsed. */
const reports = () =>
  commands.videoPlayerReport.mock.calls.map(
    ([json]) => JSON.parse(json as string) as Record<string, unknown>,
  );

const open = (json: Record<string, unknown>) =>
  emitEvent('videoPlayerEvent', { kind: 'open', json: JSON.stringify(json) });
const update = (json: Record<string, unknown>) =>
  emitEvent('videoPlayerEvent', { kind: 'update', json: JSON.stringify(json) });

const EPISODE = {
  id: 'stremio:series:tt2:1:1',
  url: 'http://127.0.0.1:1/media/t/https/cdn.example/episode.mp4',
  format: 'mp4',
  title: 'The Show',
  subtitle: 'S1E1',
  subtitles: [],
  startMs: 0,
  seekStepMs: 10_000,
  group: 'stremio:series:tt2',
  titleState: null,
  chapters: [],
};

const facts = (height: number | null, extra: Record<string, unknown> = {}) => ({
  height,
  range: 'sdr',
  codec: null,
  bitDepth: null,
  audio: null,
  sizeBytes: null,
  release: null,
  ...extra,
});

/** Alpha plays its 1080p link; Beta has 2160p HDR and 720p; Gamma failed. */
const SOURCES = {
  type: 'sources',
  playingFrom: null,
  order: [1, 0, 2],
  groups: [
    {
      name: 'Alpha',
      state: 'ready',
      error: null,
      best: 1080,
      options: 1,
      speedKbps: 9_000,
      links: [
        {
          label: 'Alpha 1080p · The.Show.S01E01.1080p.WEB-DL.x264-GROUP',
          current: true,
          failed: false,
          facts: facts(1080, { codec: 'H.264', release: 'WEB-DL' }),
          bitrateKbps: 4_500,
          speedKbps: 9_000,
        },
      ],
    },
    {
      name: 'Beta',
      state: 'ready',
      error: null,
      best: 2160,
      options: 2,
      speedKbps: null,
      worked: true,
      links: [
        {
          label: '[RD+] Beta 720p · Show.S01E01.720p.HDTV',
          current: false,
          failed: false,
          facts: facts(720),
        },
        {
          label: '[RD+] Beta 4k · The.Show.S01E01.2160p.WEB-DL.DDP5.1.HDR.H.265-GROUP 💾 15.2 GB',
          current: false,
          failed: false,
          facts: facts(2160, {
            range: 'hdr',
            codec: 'HEVC',
            audio: 'DD+ 5.1',
            sizeBytes: 15_200_000_000,
            release: 'WEB-DL',
          }),
          bitrateKbps: 20_000,
        },
      ],
    },
    { name: 'Gamma', state: 'failed', error: 'down', links: [] },
  ],
};

const startSession = async () => {
  await AppShellWrapper.mount('/');
  open({ session: true, id: '', title: 'The Show', subtitle: 'S1E1' });
  await screen.findByRole('dialog', { name: 'Video player' });
  update({ type: 'load', request: EPISODE });
  await screen.findByTestId('desktop-video');
  update(SOURCES);
  return screen.getByRole('dialog', { name: 'Video player' });
};

describe('the desktop player’s sources, quality and live streams', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    window.innerWidth = 1280;
    window.innerHeight = 720;
  });

  it('lists sources by clean names beside their options, with the raw list a tab away', async () => {
    const user = userEvent.setup();
    const player = await startSession();
    await user.click(within(player).getByRole('button', { name: 'Sources' }));
    const panel = screen.getByRole('region', { name: 'Sources' });
    expect(within(panel).getByRole('tab', { name: 'Sources' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    // Rust's order: Beta (4K) before Alpha; no long stream names in the list.
    const names = within(panel)
      .getAllByRole('menuitemradio')
      .map((item) => item.textContent ?? '');
    expect(names[0]).toMatch(/^Beta/);
    expect(names[0]).toContain('Up to 2160p · 2 options · worked last time');
    expect(names[1]).toMatch(/^Alpha/);
    expect(names[1]).toContain('Playing · Up to 1080p · 1 option · 9 Mb/s');
    expect(within(panel).queryByText(/WEB-DL\.x264-GROUP/)).not.toBeInTheDocument();
    // The source playing shows its options beside the list.
    const alpha = within(panel).getByRole('group', { name: 'Alpha' });
    expect(within(alpha).getByRole('menuitemradio', { name: /1080p/ })).toHaveTextContent(
      'H.264 · needs ~4.5 Mb/s · 9 Mb/s measured · WEB-DL · Playing',
    );

    // Another source's options, best picture first; picking one plays it from here.
    await user.click(within(panel).getByRole('menuitemradio', { name: /^Beta/ }));
    const beta = within(panel).getByRole('group', { name: 'Beta' });
    const options = within(beta).getAllByRole('menuitemradio');
    expect(options[0]).toHaveTextContent('2160p (4K) · HDR');
    expect(options[0]).toHaveTextContent('HEVC · DD+ 5.1 · 15.2 GB · needs ~20 Mb/s · WEB-DL');
    expect(options[1]).toHaveTextContent('720p');
    await user.click(options[0] as HTMLElement);
    expect(reports()).toContainEqual(
      expect.objectContaining({ state: 'request', action: 'source', group: 1, link: 1 }),
    );

    // Raw details: the sources' own names.
    await user.click(within(player).getByRole('button', { name: 'Sources' }));
    const again = screen.getByRole('region', { name: 'Sources' });
    await user.click(within(again).getByRole('tab', { name: 'Raw details' }));
    expect(
      within(again).getByRole('menuitemradio', {
        name: /The\.Show\.S01E01\.1080p\.WEB-DL\.x264-GROUP/,
      }),
    ).toBeInTheDocument();
  });

  it('goes from the list to a source’s options in a narrow window', async () => {
    const user = userEvent.setup();
    window.innerWidth = 480;
    window.innerHeight = 900;
    const player = await startSession();
    await user.click(within(player).getByRole('button', { name: 'Sources' }));
    const panel = screen.getByRole('region', { name: 'Sources' });
    expect(within(panel).queryByRole('group')).not.toBeInTheDocument();
    await user.click(within(panel).getByRole('menuitemradio', { name: /^Beta/ }));
    expect(within(panel).getByRole('group', { name: 'Beta' })).toBeInTheDocument();
    await user.click(within(panel).getByRole('button', { name: 'All sources' }));
    expect(within(panel).getByRole('menuitemradio', { name: /^Alpha/ })).toBeInTheDocument();
  });

  it('offers every resolution, from another source when this one hasn’t it', async () => {
    const user = userEvent.setup();
    const player = await startSession();
    // The bar: quality, then subtitles, then the picture's aspect.
    const bar = within(player).getByRole('button', { name: 'Quality' })
      .parentElement as HTMLElement;
    const order = within(bar)
      .getAllByRole('button')
      .map((button) => button.getAttribute('aria-label'));
    const quality = order.indexOf('Quality');
    expect(order.slice(quality, quality + 3)).toEqual(['Quality', 'Subtitles', 'Aspect']);

    update({
      type: 'qualities',
      wanted: null,
      notice: null,
      items: [
        {
          key: '2160:hdr',
          height: 2160,
          range: 'hdr',
          codec: 'HEVC',
          state: 'otherSource',
          group: 1,
          link: 1,
          source: 'Beta',
        },
        // A cropped film: its tier, with its own size.
        {
          key: '1080:sdr',
          height: 1080,
          range: 'sdr',
          codec: 'H.264',
          exact: '1920×816',
          sizes: [],
          state: 'playing',
        },
        {
          key: '720:sdr',
          height: 720,
          range: 'sdr',
          codec: null,
          state: 'thisSource',
          group: 0,
          link: 1,
        },
      ],
    });
    await user.click(within(player).getByRole('button', { name: 'Quality' }));
    const panel = screen.getByRole('region', { name: 'Quality' });
    expect(
      within(panel).getByRole('menuitemradio', { name: /2160p \(4K\) · HDR/ }),
    ).toHaveTextContent('Not in this source · plays from Beta');
    // Only the tiers the sources have are listed.
    expect(within(panel).queryByRole('menuitemradio', { name: /1440p/ })).not.toBeInTheDocument();
    expect(
      within(panel).getByRole('menuitemradio', { name: /1080p \(1920×816\)/ }),
    ).toHaveTextContent('Playing · H.264');
    await user.click(within(panel).getByRole('menuitemradio', { name: /2160p/ }));
    expect(reports()).toContainEqual(
      expect.objectContaining({ state: 'request', action: 'quality', key: '2160:hdr' }),
    );
    expect(screen.getByText('Loading Beta · 2160p (4K) · HDR…')).toBeInTheDocument();

    // Tapping the step that plays asks for the next source with it; Rust may say there's none.
    await user.click(within(player).getByRole('button', { name: 'Quality' }));
    await user.click(
      within(screen.getByRole('region', { name: 'Quality' })).getByRole('menuitemradio', {
        name: /1080p/,
      }),
    );
    expect(reports()).toContainEqual(
      expect.objectContaining({ action: 'quality', key: '1080:sdr' }),
    );
    act(() => {
      update({ type: 'qualities', items: [], notice: 'No other source has 1080p' });
    });
    expect(await screen.findByText('No other source has 1080p')).toBeInTheDocument();
  });

  it('plays a live stream from its edge, without a place to resume, and holds it for AI subtitles', async () => {
    const user = userEvent.setup();
    commands.captions.mockImplementation(async (episodeId: string) =>
      ok({
        episodeId,
        cues: [],
        source: null,
        covered: [],
        complete: false,
        running: true,
        preparing: false,
        error: null,
        canTranscribe: true,
        engine: 'local',
        hasFeed: false,
        queued: null,
        durationMs: null,
        liveDelayMs: 50_000,
      }),
    );
    await AppShellWrapper.mount('/');
    open({ ...EPISODE, id: 'stremio:tv:news', startMs: 61_000, live: true });
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const video = (await within(player).findByTestId('desktop-video')) as HTMLVideoElement;
    Object.defineProperty(video, 'duration', { value: Infinity, configurable: true });
    Object.defineProperty(video, 'seekable', {
      value: { length: 1, start: () => 3_000, end: () => 3_600 },
      configurable: true,
    });
    fireEvent(video, new Event('loadedmetadata'));
    // Not the place asked for: a live stream starts where its engine starts it.
    expect(video.currentTime).toBe(0);
    video.currentTime = 3_590;
    fireEvent(video, new Event('timeupdate'));

    expect(
      await within(player).findByRole('button', { name: 'Go to the live edge' }),
    ).toHaveTextContent('LIVE');
    fireEvent(video, new Event('pause'));
    expect(reports().at(-1)).toMatchObject({
      id: 'stremio:tv:news',
      state: 'progress',
      live: true,
      durationMs: null,
    });

    // AI subtitles are being made: they need the picture 50 s behind live.
    const offer = await screen.findByText('AI subtitles need the picture 50 s behind live');
    await user.click(
      within(offer.parentElement as HTMLElement).getByRole('button', { name: 'Delay' }),
    );
    expect(video.currentTime).toBe(3_550);
  });
});
