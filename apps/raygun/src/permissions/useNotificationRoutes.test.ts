import { splitRoute } from './useNotificationRoutes';

describe('notification routes', () => {
  it('split into a path and its search', () => {
    expect(splitRoute('/downloads?tab=music')).toEqual({
      to: '/downloads',
      search: { tab: 'music' },
    });
    expect(splitRoute('/podcast?id=abc')).toEqual({ to: '/podcast', search: { id: 'abc' } });
    expect(splitRoute('/downloads')).toEqual({ to: '/downloads', search: {} });
  });
});
