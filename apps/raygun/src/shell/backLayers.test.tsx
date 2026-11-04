import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import type { FC } from 'react';

import { afterLayersClose, backFromLayers, resetBackLayers, useBackLayer } from './backLayers';

/** A sheet that opens a menu inside it: two layers, one over the other. */
const Layers: FC = () => {
  const [sheet, setSheet] = useState(false);
  const [menu, setMenu] = useState(false);
  useBackLayer(sheet, () => setSheet(false));
  useBackLayer(sheet && menu, () => setMenu(false));
  return (
    <>
      <button type="button" onClick={() => setSheet(true)}>
        Open sheet
      </button>
      {sheet && (
        <section aria-label="Sheet">
          <button type="button" onClick={() => setMenu(true)}>
            Open menu
          </button>
          <button type="button" onClick={() => setSheet(false)}>
            Done
          </button>
          {menu && <section aria-label="Menu">Menu</section>}
        </section>
      )}
    </>
  );
};

const marked = () =>
  typeof window.history.state === 'object' &&
  window.history.state !== null &&
  'raygunLayer' in window.history.state;

const region = (name: string) => screen.queryByRole('region', { name });

/** Opens the sheet and its menu, and waits for their history entries. */
const openBoth = async () => {
  const user = userEvent.setup();
  render(<Layers />);
  const before = window.history.length;
  await user.click(screen.getByRole('button', { name: 'Open sheet' }));
  await user.click(screen.getByRole('button', { name: 'Open menu' }));
  return { user, before };
};

describe('Back layers on a phone (history entries)', () => {
  beforeEach(() => {
    resetBackLayers(true);
    window.history.replaceState({ page: 'start' }, '');
  });

  it('closes the innermost layer first, then the one under it, one entry each', async () => {
    const { before } = await openBoth();
    await waitFor(() => expect(window.history.length).toBe(before + 2));
    expect(marked()).toBe(true);

    window.history.back();
    await waitFor(() => expect(region('Menu')).not.toBeInTheDocument());
    expect(region('Sheet')).toBeInTheDocument();

    window.history.back();
    await waitFor(() => expect(region('Sheet')).not.toBeInTheDocument());
    // Nothing left over: the page's own entry is current again.
    await waitFor(() => expect(window.history.state).toEqual({ page: 'start' }));
  });

  it('takes the entry along when a layer closes another way', async () => {
    const user = userEvent.setup();
    render(<Layers />);
    await user.click(screen.getByRole('button', { name: 'Open sheet' }));
    await waitFor(() => expect(marked()).toBe(true));

    await user.click(screen.getByRole('button', { name: 'Done' }));

    await waitFor(() => expect(window.history.state).toEqual({ page: 'start' }));
    expect(region('Sheet')).not.toBeInTheDocument();
  });

  it('closes only the top layer on Escape', async () => {
    const { user } = await openBoth();
    await waitFor(() => expect(marked()).toBe(true));

    await user.keyboard('{Escape}');
    await waitFor(() => expect(region('Menu')).not.toBeInTheDocument());
    expect(region('Sheet')).toBeInTheDocument();

    await user.keyboard('{Escape}');
    await waitFor(() => expect(region('Sheet')).not.toBeInTheDocument());
    await waitFor(() => expect(window.history.state).toEqual({ page: 'start' }));
  });

  it('runs what a closing menu chose once its entry has left history', async () => {
    const user = userEvent.setup();
    render(<Layers />);
    await user.click(screen.getByRole('button', { name: 'Open sheet' }));
    await waitFor(() => expect(marked()).toBe(true));

    // A menu item that opens a page: the layer closes, then the page opens.
    const seen: unknown[] = [];
    await user.click(screen.getByRole('button', { name: 'Done' }));
    afterLayersClose(() => {
      seen.push(window.history.state);
      window.history.pushState({ page: 'next' }, '');
    });
    expect(seen).toEqual([]);
    await waitFor(() => expect(seen).toHaveLength(1));
    // It ran on the page's own entry, so the new page took the layer's place.
    expect(seen[0]).toEqual({ page: 'start' });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(window.history.state).toEqual({ page: 'next' });
    // Back from it lands on the page, not on the closed layer's entry.
    window.history.back();
    await waitFor(() => expect(window.history.state).toEqual({ page: 'start' }));
  });

  it('steps over the entry of a layer a link closed', async () => {
    const user = userEvent.setup();
    render(<Layers />);
    await user.click(screen.getByRole('button', { name: 'Open sheet' }));
    await waitFor(() => expect(marked()).toBe(true));

    // A link from the sheet opens a page over its entry, and the sheet closes for it.
    window.history.pushState({ page: 'next' }, '');
    await user.click(screen.getByRole('button', { name: 'Done' }));
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(window.history.state).toEqual({ page: 'next' });

    // Back from that page lands where the sheet was opened, not on the sheet's old entry.
    window.history.back();
    await waitFor(() => expect(window.history.state).toEqual({ page: 'start' }));
  });
});

describe('Back layers on desktop (no history entries)', () => {
  beforeEach(() => {
    resetBackLayers(false);
  });

  it('runs what a closing menu chose at once', () => {
    const run = vi.fn();
    afterLayersClose(run);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it('closes layers innermost first before going back a page, and keeps history alone', async () => {
    const { before } = await openBoth();
    const back = vi.fn();

    backFromLayers(back);
    await waitFor(() => expect(region('Menu')).not.toBeInTheDocument());
    expect(region('Sheet')).toBeInTheDocument();
    backFromLayers(back);
    await waitFor(() => expect(region('Sheet')).not.toBeInTheDocument());
    expect(back).not.toHaveBeenCalled();

    backFromLayers(back);
    expect(back).toHaveBeenCalledTimes(1);
    expect(window.history.length).toBe(before);
  });
});
