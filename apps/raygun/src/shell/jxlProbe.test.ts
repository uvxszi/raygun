import { renderHook, waitFor } from '@testing-library/react';

import { bindingsMock } from '../test/bindingsMock';
import { JXL_PROBE, showsJxl, useJxlProbe } from './jxlProbe';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

/** Pictures load at `width` pixels wide, or fail when `width` is null, as a webview would. */
const webviewShows = (width: number | null) =>
  vi.spyOn(HTMLImageElement.prototype, 'src', 'set').mockImplementation(function (
    this: HTMLImageElement,
  ) {
    if (width === null) {
      queueMicrotask(() => this.dispatchEvent(new Event('error')));
      return;
    }
    Object.defineProperty(this, 'naturalWidth', { value: width });
    queueMicrotask(() => this.dispatchEvent(new Event('load')));
  });

describe('The JPEG XL probe', () => {
  beforeEach(() => commands.coversWebviewJxl.mockClear());
  afterEach(() => vi.restoreAllMocks());

  it('is a JPEG XL picture in the container, as covers are stored', () => {
    const [kind, data = ''] = JXL_PROBE.split(',');
    expect(kind).toBe('data:image/jxl;base64');
    const bytes = Uint8Array.from(atob(data), (char) => char.charCodeAt(0));
    expect([...bytes.slice(0, 12)]).toEqual([
      0, 0, 0, 0x0c, 0x4a, 0x58, 0x4c, 0x20, 0x0d, 0x0a, 0x87, 0x0a,
    ]);
  });

  it('tells Rust the webview shows JPEG XL when the picture shows', async () => {
    webviewShows(8);
    expect(await showsJxl()).toBe(true);
    renderHook(() => useJxlProbe());
    await waitFor(() => expect(commands.coversWebviewJxl).toHaveBeenCalledWith(true));
  });

  it("tells Rust it doesn't when the picture fails", async () => {
    webviewShows(null);
    renderHook(() => useJxlProbe());
    await waitFor(() => expect(commands.coversWebviewJxl).toHaveBeenCalledWith(false));
    expect(commands.coversWebviewJxl).not.toHaveBeenCalledWith(true);
  });
});
