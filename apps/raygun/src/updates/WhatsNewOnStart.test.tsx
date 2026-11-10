import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock, seedBindings } from '../test/bindingsMock';
import { WhatsNewOnStart } from './WhatsNewSheet';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const NOTES = { version: '1.3.1', notes: '- Faster starts' };

const later = <T,>() => {
  let resolve: (value: T) => void = () => {};
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
};

const mount = (client: QueryClient) =>
  render(
    <QueryClientProvider client={client}>
      <WhatsNewOnStart />
    </QueryClientProvider>,
  );

describe("What's new on start", () => {
  beforeEach(() => {
    seedBindings();
  });

  it('never shows notes an earlier run saved, while Rust says whether they were seen', async () => {
    const answer = later<ReturnType<typeof ok<null>>>();
    commands.appWhatsNew.mockReturnValue(answer.promise);
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    // Last run's answer, restored from the saved cache and marked out of date.
    client.setQueryData(['appWhatsNew'], NOTES, { updatedAt: Date.now() - 60_000 });
    void client.invalidateQueries({ refetchType: 'none' });

    mount(client);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    answer.resolve(ok(null));
    await waitFor(() => expect(commands.appWhatsNew).toHaveBeenCalled());
    await waitFor(() => expect(client.getQueryData(['appWhatsNew'])).toBeNull());
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it("shows this run's answer, and forgets it once closed", async () => {
    const user = userEvent.setup();
    commands.appWhatsNew.mockResolvedValue(ok(NOTES));
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    mount(client);

    const sheet = await screen.findByRole('dialog', { name: "What's new in 1.3.1" });
    await user.click(screen.getByRole('button', { name: 'Close' }));
    expect(sheet).not.toBeInTheDocument();
    expect(commands.appWhatsNewSeen).toHaveBeenCalled();
    // Nothing left to save or show again.
    expect(client.getQueryData(['appWhatsNew'])).toBeNull();
  });
});
