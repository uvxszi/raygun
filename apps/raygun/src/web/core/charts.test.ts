import type { ExtensionsService } from '../extensions/service';
import { ChartsService, chartsOf } from './charts';

const PROVIDER = {
  id: 'dev.test/charts',
  extensionId: 'dev.test',
  kind: 'music.charts',
  name: 'Test charts',
  searchPill: null,
  adult: false,
};

describe('charts in the browser', () => {
  it('reads charts as the app’s host does', () => {
    expect(
      chartsOf(PROVIDER, {
        charts: [
          { id: 1, name: ' Top ' },
          { id: 'jazz', name: 'Jazz', tag: 'jazz' },
          { id: 'x', name: 'Odd', group: 'weird', artworkUrl: 'ftp://no' },
          { name: 'No id' },
        ],
      }).map(({ id, name, group, artworkUrl }) => ({ id, name, group, artworkUrl })),
    ).toEqual([
      { id: '1', name: 'Top', group: 'top', artworkUrl: null },
      { id: 'jazz', name: 'Jazz', group: 'genre', artworkUrl: null },
      { id: 'x', name: 'Odd', group: 'top', artworkUrl: null },
    ]);
  });

  it('gathers every source’s charts and a chart’s songs', async () => {
    const extensions = {
      listed: async () => [{ info: PROVIDER, enabled: true }],
      call: async (_id: string, method: string) =>
        method === 'charts'
          ? { charts: [{ id: 'top', name: 'Top' }] }
          : { tracks: [{ title: 'Song', artist: 'Someone' }, { artist: 'No title' }] },
    } as unknown as ExtensionsService;
    const charts = new ChartsService(extensions);
    expect((await charts.charts(null)).charts.map((chart) => chart.name)).toEqual(['Top']);
    expect(
      (await charts.chart('dev.test/charts', 'top', null)).map((track) => track.title),
    ).toEqual(['Song']);
  });
});
