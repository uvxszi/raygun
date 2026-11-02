import type { MadeForYou, Mix, MixSummary, OnlineTrack } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const song = (title: string, artist: string): OnlineTrack => ({
  externalId: null,
  title,
  artist,
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  source: null,
});

const summary = (mix: Partial<MixSummary> & Pick<MixSummary, 'id' | 'kind'>): MixSummary => ({
  seed: null,
  number: null,
  artists: ['Nina Simone', 'Etta James'],
  artworkUrls: [],
  trackCount: 12,
  ...mix,
});

const overview = (mixes: MixSummary[], making = false): MadeForYou => ({
  mixes,
  stats: {
    playsThisWeek: 30,
    minutesThisWeek: 95,
    topArtist: 'Nina Simone',
    topSong: song('Sinnerman', 'Nina Simone'),
    peakHour: 21,
    songsKnown: 80,
  },
  madeAt: 1,
  making,
});

const MIXES = [
  summary({ id: 'daily-1', kind: 'daily', number: 1, seed: 'Jazz' }),
  summary({ id: 'discover-weekly', kind: 'discoverWeekly' }),
  summary({ id: 'on-repeat', kind: 'onRepeat' }),
  summary({ id: 'because-nina simone', kind: 'becauseYouListened', seed: 'Nina Simone' }),
  summary({ id: 'genre-jazz', kind: 'genre', seed: 'Jazz' }),
];

describe('Made for You', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('shows mixes by shelf and the listening numbers', async () => {
    commands.madeForYou.mockResolvedValue(ok(overview(MIXES)));
    await AppShellWrapper.mount('/music/for-you');
    const page = await screen.findByTestId('made-for-you-page');
    const daily = await within(page).findByRole('region', { name: 'Your Daily Mixes' });
    expect(within(daily).getAllByText('Daily Mix 1')).not.toHaveLength(0);
    const made = within(page).getByRole('region', { name: 'Made for you' });
    expect(within(made).getByText('Discover Weekly')).toBeInTheDocument();
    expect(within(made).getByText('On Repeat')).toBeInTheDocument();
    expect(within(page).getByText('Because you listened to Nina Simone')).toBeInTheDocument();
    expect(within(page).getByText('Jazz Mix')).toBeInTheDocument();
    const stats = within(page).getByRole('region', { name: 'Your listening' });
    expect(stats).toHaveTextContent('95 minutes');
    expect(stats).toHaveTextContent('Sinnerman');
    expect(commands.madeForYou).toHaveBeenCalledWith(false, expect.any(Number));
  });

  it('makes new mixes on request and while they are made says so', async () => {
    const user = userEvent.setup();
    commands.madeForYou.mockResolvedValue(ok(overview([], true)));
    await AppShellWrapper.mount('/music/for-you');
    expect(await screen.findByText('Making your mixes from what you play…')).toBeInTheDocument();

    commands.madeForYou.mockResolvedValue(ok(overview(MIXES)));
    emitEvent('madeForYouEvent', null);
    expect(await screen.findByText('Discover Weekly')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Make new mixes' }));
    expect(commands.madeForYou).toHaveBeenLastCalledWith(true, expect.any(Number));
  });

  it('plays a mix and saves it as a playlist', async () => {
    const user = userEvent.setup();
    const mix: Mix = {
      id: 'discover-weekly',
      kind: 'discoverWeekly',
      seed: null,
      number: null,
      artists: ['Etta James'],
      artworkUrls: [],
      tracks: [song('At Last', 'Etta James'), song('Feeling Good', 'Nina Simone')],
    };
    commands.madeForYouMix.mockResolvedValue(ok(mix));
    await AppShellWrapper.mount('/music/mix?id=discover-weekly');
    const page = await screen.findByTestId('mix-page');
    expect(await within(page).findByRole('heading', { level: 1 })).toHaveTextContent(
      'Discover Weekly',
    );
    await user.click(within(page).getByRole('button', { name: 'Play all' }));
    expect(commands.playbackReplace).toHaveBeenCalledWith(
      [
        expect.objectContaining({ title: 'At Last' }),
        expect.objectContaining({ title: 'Feeling Good' }),
      ],
      0,
    );
    await user.click(within(page).getByRole('button', { name: 'Save as playlist' }));
    expect(commands.madeForYouSave).toHaveBeenCalledWith('discover-weekly', 'Discover Weekly');
    expect(await within(page).findByText('Saved to your playlists.')).toBeInTheDocument();
  });

  it('puts the mixes on Home and in Browse all', async () => {
    commands.madeForYou.mockResolvedValue(ok(overview(MIXES)));
    await AppShellWrapper.mount('/');
    const row = await screen.findByTestId('made-for-you-row');
    expect(within(row).getAllByTestId('mix-card')).toHaveLength(MIXES.length);
    expect(within(row).getByRole('link', { name: 'See all' })).toBeInTheDocument();
  });

  it('says how mixes start when there is nothing to make them from', async () => {
    await AppShellWrapper.mount('/music/for-you');
    expect(await screen.findByText('Your mixes start here')).toBeInTheDocument();
  });
});
