import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { setRemoteControls, setRemoteState } from '../web/remoteState';
import { WebBackendCard, WebBackendVia } from './WebBackend';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const OFF = {
  available: false,
  enabled: false,
  deviceId: null,
  device: null,
  connected: false,
  connecting: false,
  error: null,
};

describe('WebBackend', () => {
  beforeEach(() => {
    seedBindings();
    commands.appInfo.mockResolvedValue({
      name: 'Raygun',
      version: '1.0.0',
      platform: 'web',
      arch: 'browser',
      debug: false,
    });
    setRemoteState(OFF);
  });

  afterEach(() => {
    setRemoteControls(null);
    setRemoteState(OFF);
  });

  it('shows the app in use and switches it off', async () => {
    const setEnabled = vi.fn();
    setRemoteControls({ setEnabled, choose: vi.fn() });
    setRemoteState({
      available: true,
      enabled: true,
      deviceId: 'laptop',
      device: 'Laptop',
      connected: true,
    });
    renderWithProviders(
      <>
        <WebBackendCard />
        <WebBackendVia />
      </>,
    );
    const toggle = await screen.findByRole('switch', { name: /Use Laptop/ });
    expect(toggle).toHaveTextContent('In use');
    expect(screen.getByTestId('web-backend-via')).toHaveTextContent('via Laptop');
    await userEvent.click(toggle);
    expect(setEnabled).toHaveBeenCalledWith(false);
  });

  it('says why the app could not be reached, and shows nothing without one', async () => {
    setRemoteState({
      available: true,
      enabled: true,
      device: 'Laptop',
      error: 'Couldn’t reach Laptop',
    });
    const { unmount } = renderWithProviders(<WebBackendCard />);
    expect(await screen.findByText('Not reachable: Couldn’t reach Laptop')).toBeInTheDocument();
    unmount();
    setRemoteState(OFF);
    renderWithProviders(<WebBackendCard />);
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });
});
