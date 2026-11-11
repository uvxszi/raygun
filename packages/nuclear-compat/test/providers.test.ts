import { CONNECT_HINT, explainError, wrapProvider } from '../src/shim/providers';
import type { NuclearProvider } from '../src/shim/nuclear-types';

describe('errors of plugins without a server', () => {
  it('point to Sources → Add when a plugin says it is not connected', () => {
    const explained = explainError(new Error('Subsonic not connected')) as Error;
    expect(explained.message).toBe(`Subsonic not connected. ${CONNECT_HINT}`);
    expect(explained.message).toContain('Sources → Add');
  });

  it('read "no server" and missing credentials the same way', () => {
    expect((explainError('No server URL configured') as Error).message).toContain(CONNECT_HINT);
    expect((explainError(new Error('Password is required')) as Error).message).toContain(
      CONNECT_HINT,
    );
  });

  it('leave other errors as they are', () => {
    const error = new Error('Request failed with status 500');
    expect(explainError(error)).toBe(error);
  });

  it('say it once', () => {
    const once = explainError(new Error('Not connected')) as Error;
    expect(explainError(once)).toBe(once);
  });

  it("explain a converted provider's failure", async () => {
    const provider = {
      kind: 'metadata',
      id: 'subsonic',
      name: 'Subsonic',
      searchTracks: async () => {
        throw new Error('Subsonic not connected');
      },
    } as unknown as NuclearProvider;
    const wrapped = wrapProvider(provider) as { search: (query: unknown) => Promise<unknown> };
    await expect(wrapped.search({ query: 'song', limit: 5 })).rejects.toThrow(CONNECT_HINT);
  });
});
