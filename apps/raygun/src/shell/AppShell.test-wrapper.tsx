import type { PlayContext, QueueItem } from '@raygun/bindings';
import { RouterProvider, createMemoryHistory } from '@tanstack/react-router';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { resetInstallQueue } from '../extensions/installQueue';
import { usePlayerUiStore } from '../player/playerUiStore';
import { createAppRouter } from '../router';
import { resetSourceQueue } from '../sources/sourceQueue';
import { forgetLists } from './listMemory';
import { EMPTY_QUEUE, IDLE_STATE, emitEvent, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { useTrailerStore } from '../video/trailerStore';
import { useVideoPlayerStore } from '../video/videoPlayerStore';

export const makeItem = (title: string, artist: string | null = null): QueueItem => ({
  id: `id-${title}`,
  title,
  artist,
  album: null,
  durationMs: 180_000,
  artworkUrl: null,
  mediaKind: 'audio',
  source: { kind: 'url', url: `https://example.com/${title}.mp3`, headers: {} },
});

export const AppShellWrapper = {
  seedCore() {
    seedBindings();
    usePlayerUiStore.setState({
      sheetOpen: false,
      sheetView: 'artwork',
      videoShown: false,
      panel: null,
      unmutedVolume: 1,
    });
    useVideoPlayerStore.getState().close();
    useTrailerStore.getState().close();
    resetInstallQueue();
    resetSourceQueue();
    forgetLists();
  },

  async mount(path = '/') {
    const router = createAppRouter(createMemoryHistory({ initialEntries: [path] }));
    renderWithProviders(<RouterProvider router={router} />);
    await screen.findByTestId('page-content');
    return router;
  },

  /** Simulates Rust pushing a queue where the item at `cursor` is current. */
  playing(
    items: QueueItem[],
    cursor: number,
    status: 'playing' | 'paused' = 'playing',
    context: PlayContext | null = null,
  ) {
    emitEvent('queueEvent', {
      ...EMPTY_QUEUE,
      items,
      order: items.map((_, index) => index),
      context,
    });
    emitEvent('playbackStateEvent', {
      ...IDLE_STATE,
      status,
      currentItemId: items[cursor]?.id ?? null,
      positionMs: 65_000,
      durationMs: 180_000,
    });
  },

  page(name: 'home' | 'library' | 'search' | 'settings' | 'browse' | 'more') {
    return screen.findByTestId(`${name}-page`);
  },

  async tapTab(name: 'home' | 'library' | 'browse' | 'more') {
    await userEvent.click(screen.getByTestId(`nav-${name}-tab`));
  },

  get activeTab() {
    return within(screen.getByTestId('bottom-tabs')).getByRole('link', { current: 'page' });
  },

  miniPlayer: {
    get element() {
      return screen.getByTestId('mini-player');
    },
    get title() {
      return screen.findByTestId('mini-player-title');
    },
    async open() {
      // The bar appears once the queue has loaded with something in it.
      await userEvent.click(await screen.findByRole('button', { name: 'Open player' }));
    },
    async togglePlayback(label: 'Play' | 'Pause') {
      await userEvent.click(within(this.element).getByRole('button', { name: label }));
    },
  },

  playerSheet: {
    get element() {
      return screen.findByRole('dialog', { name: 'Now playing' });
    },
    async press(label: string) {
      const sheet = await this.element;
      await userEvent.click(within(sheet).getByRole('button', { name: label }));
    },
    get upNext() {
      return screen.findByTestId('up-next');
    },
    async close() {
      await userEvent.click(screen.getByRole('button', { name: 'Close player' }));
    },
    async waitUntilClosed() {
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    },
  },
};
