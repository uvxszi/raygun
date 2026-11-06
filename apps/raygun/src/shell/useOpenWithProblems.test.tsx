import { screen } from '@testing-library/react';
import type { FC } from 'react';

import { emitEvent, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { ToastHost } from './Toast';
import { useOpenWithProblems } from './useOpenWithProblems';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const Host: FC = () => {
  useOpenWithProblems();
  return <ToastHost />;
};

describe('useOpenWithProblems', () => {
  beforeEach(() => {
    seedBindings();
  });

  it('says what another app opened with Raygun that did not play', async () => {
    renderWithProviders(<Host />);

    emitEvent('openWithEvent', { kind: 'unreadable', name: 'Song.mp3' });
    expect(await screen.findByRole('status')).toHaveTextContent('Couldn’t open Song.mp3');

    emitEvent('openWithEvent', { kind: 'emptyPlaylist', name: 'Trip.m3u' });
    expect(await screen.findByText('Trip.m3u has nothing Raygun can play')).toBeInTheDocument();

    emitEvent('openWithEvent', { kind: 'unsupported', name: 'notes.pdf' });
    expect(await screen.findByText('Raygun can’t play notes.pdf')).toBeInTheDocument();
  });
});
