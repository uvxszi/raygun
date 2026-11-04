import type { McpStatus } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const ON: McpStatus = {
  enabled: true,
  url: 'http://127.0.0.1:8810/mcp',
  token: 'token-1',
  error: null,
};

describe('MCP server card', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('turns the server on and shows its address and token', async () => {
    const user = userEvent.setup();
    commands.mcpSetEnabled.mockResolvedValue(ok(ON));
    await AppShellWrapper.mount('/settings/ai');
    const card = await screen.findByTestId('mcp-card');

    const toggle = await within(card).findByRole('switch', { name: 'MCP server' });
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    expect(within(card).queryByTestId('mcp-url')).not.toBeInTheDocument();

    await user.click(toggle);
    expect(commands.mcpSetEnabled).toHaveBeenCalledWith(true);
    expect(await within(card).findByTestId('mcp-url')).toHaveTextContent(ON.url as string);
    expect(toggle).toHaveAttribute('aria-checked', 'true');

    expect(within(card).getByTestId('mcp-token')).not.toHaveTextContent('token-1');
    await user.click(within(card).getByRole('button', { name: 'Show token' }));
    expect(within(card).getByTestId('mcp-token')).toHaveTextContent('token-1');

    await user.click(within(card).getByRole('button', { name: 'Copy token' }));
    expect(await navigator.clipboard.readText()).toBe('token-1');
  });

  it('issues a new token', async () => {
    const user = userEvent.setup();
    commands.mcpStatus.mockResolvedValue(ok(ON));
    commands.mcpNewToken.mockResolvedValue(ok({ ...ON, token: 'token-2' }));
    await AppShellWrapper.mount('/settings/ai');
    const card = await screen.findByTestId('mcp-card');

    await user.click(await within(card).findByRole('button', { name: 'Show token' }));
    await user.click(within(card).getByRole('button', { name: 'New token' }));
    expect(commands.mcpNewToken).toHaveBeenCalled();
    expect(await within(card).findByText('token-2')).toBeInTheDocument();
  });

  it('says why the server is not running', async () => {
    commands.mcpStatus.mockResolvedValue(
      ok({ ...ON, url: null, error: 'Couldn’t start: address in use' }),
    );
    await AppShellWrapper.mount('/settings/ai');
    const card = await screen.findByTestId('mcp-card');
    const problem = await within(card).findByRole('alert');
    expect(problem).toHaveTextContent("The MCP server isn't running");
    await userEvent.click(within(problem).getByRole('button', { name: 'More info' }));
    expect(problem).toHaveTextContent('address in use');
    expect(within(card).getByTestId('mcp-url')).toHaveTextContent('Not running');
  });
});
