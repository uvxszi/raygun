import { toRaygunSetting } from '../src/shim/api';
import type { NuclearProvider } from '../src/shim/nuclear-types';
import { asSetupError, wrapProvider } from '../src/shim/providers';

/** What Raygun's prelude throws for `raygun.needsSetup` (the mark the host reads). */
const MARK = '[raygun:needs-setup]';

beforeEach(() => {
  (globalThis as { raygun?: unknown }).raygun = {
    needsSetup: (reason: string) => {
      throw new Error(`${MARK} ${reason}`);
    },
  };
});

afterEach(() => {
  delete (globalThis as { raygun?: unknown }).raygun;
});

const messageOf = (error: unknown) => (error instanceof Error ? error.message : String(error));

describe('errors of plugins without a server, key or sign-in', () => {
  it('say the plugin needs setup when it says it is not connected', () => {
    expect(messageOf(asSetupError(new Error('Subsonic not connected')))).toBe(
      `${MARK} Subsonic not connected`,
    );
  });

  it('read "no server", missing credentials and refused keys the same way', () => {
    for (const said of [
      'No server URL configured',
      'Password is required',
      'Invalid API key',
      'Request failed: Unauthorized',
    ]) {
      expect(messageOf(asSetupError(said))).toContain(MARK);
    }
  });

  it('leave other errors as they are', () => {
    const error = new Error('Request failed with status 500');
    expect(asSetupError(error)).toBe(error);
  });

  it('leave them as they are where the host has no needsSetup', () => {
    (globalThis as { raygun?: unknown }).raygun = {};
    const error = new Error('Not connected');
    expect(asSetupError(error)).toBe(error);
  });

  it("mark a converted provider's failure", async () => {
    const provider = {
      kind: 'metadata',
      id: 'subsonic',
      name: 'Subsonic',
      searchTracks: async () => {
        throw new Error('Subsonic not connected');
      },
    } as unknown as NuclearProvider;
    const wrapped = wrapProvider(provider) as { search: (query: unknown) => Promise<unknown> };
    await expect(wrapped.search({ query: 'song', limit: 5 })).rejects.toThrow(MARK);
  });
});

describe("a plugin's settings on its page", () => {
  it('shows text, number, switch and choice settings, a password left empty as required', () => {
    expect(
      toRaygunSetting({
        id: 'password',
        title: 'Password',
        kind: 'string',
        widget: { type: 'password' },
      }),
    ).toEqual({
      key: 'password',
      type: 'text',
      label: 'Password',
      options: [],
      default: null,
      required: true,
    });
    expect(
      toRaygunSetting({ id: 'server', title: 'Server', kind: 'string', format: 'url' }),
    ).toMatchObject({ type: 'text', required: false });
    expect(
      toRaygunSetting({
        id: 'quality',
        kind: 'enum',
        options: [{ value: 'high' }, { value: 'low' }],
        default: 'high',
      }),
    ).toMatchObject({ type: 'select', options: ['high', 'low'], default: 'high' });
    expect(toRaygunSetting({ id: 'scrobble', kind: 'boolean', default: true })).toMatchObject({
      type: 'boolean',
      required: false,
    });
    // A token with a default works without one.
    expect(
      toRaygunSetting({ id: 'token', kind: 'string', format: 'token', default: 'shared' }),
    ).toMatchObject({ required: false });
  });

  it('leaves out hidden settings and those drawn by the plugin itself', () => {
    expect(toRaygunSetting({ id: 'volume', kind: 'number', hidden: true })).toBeNull();
    expect(toRaygunSetting({ id: 'auth', kind: 'custom' })).toBeNull();
    expect(toRaygunSetting({ id: 'empty', kind: 'enum', options: [] })).toBeNull();
  });
});
