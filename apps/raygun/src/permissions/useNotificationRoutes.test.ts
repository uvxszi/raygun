import { routeSteps, splitRoute } from './useNotificationRoutes';

describe('notification routes', () => {
  it('split into a path and its search', () => {
    expect(splitRoute('/downloads?tab=music')).toEqual({
      to: '/downloads',
      search: { tab: 'music' },
    });
    expect(splitRoute('/podcast?id=abc')).toEqual({ to: '/podcast', search: { id: 'abc' } });
    expect(splitRoute('/downloads')).toEqual({ to: '/downloads', search: {} });
    // The video player's link to a settings section.
    expect(splitRoute('/settings/ai#transcription')).toEqual({
      to: '/settings/ai',
      search: {},
      hash: 'transcription',
    });
  });

  it('open screens one over another, each with the row to show there', () => {
    expect(
      routeSteps('/artist?name=Nina+Simone+%26+Friends\n/music/album?provider=dz&id=a%2F1'),
    ).toEqual([
      { to: '/artist', search: { name: 'Nina Simone & Friends' }, reveal: null },
      { to: '/music/album', search: { provider: 'dz', id: 'a/1' }, reveal: null },
    ]);
    expect(routeSteps('/artist?name=Nina+Simone&reveal=release%3Adz%2Fs1')).toEqual([
      { to: '/artist', search: { name: 'Nina Simone' }, reveal: 'release:dz/s1' },
    ]);
    expect(routeSteps('/downloads?tab=music\n')).toEqual([
      { to: '/downloads', search: { tab: 'music' }, reveal: null },
    ]);
  });
});
