import type { AiModel, AiProvider } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const CLAUDE: AiProvider = {
  id: 'p1',
  kind: 'claude',
  name: 'Claude',
  baseUrl: null,
  model: null,
  hasKey: true,
  keyHint: 'a1b2',
};

const MODEL: AiModel = {
  id: 'realcugan-up2x-conservative',
  name: 'Real-CUGAN 2× (conservative)',
  kind: 'upscale',
  description: 'Doubles images.',
  license: 'MIT',
  licenseUrl: 'https://example.com/license',
  sourceUrl: 'https://example.com',
  sizeBytes: 2_578_409,
  installed: false,
  downloading: null,
};

describe('AI settings', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('summarises translators and models on the settings page', async () => {
    commands.aiProviders.mockResolvedValue(ok([CLAUDE]));
    commands.aiModels.mockResolvedValue([{ ...MODEL, installed: true }]);
    await AppShellWrapper.mount('/settings');
    const card = await screen.findByTestId('ai-card');
    expect(await within(card).findByText('1 translator · 1 models installed')).toBeInTheDocument();
    expect(card).toHaveAttribute('href', '/settings/ai');
  });

  it('lists translators with their key hints and tests one', async () => {
    const user = userEvent.setup();
    commands.aiProviders.mockResolvedValue(ok([CLAUDE]));
    commands.aiProviderTest.mockResolvedValue(ok('おはようございます！'));
    await AppShellWrapper.mount('/settings/ai');

    const row = await screen.findByRole('listitem', { name: 'Claude' });
    expect(within(row).getByText('Claude · key ••••a1b2')).toBeInTheDocument();
    await user.click(within(row).getByRole('button', { name: /^Test/ }));
    expect(commands.aiProviderTest).toHaveBeenCalledWith('p1');
    expect(await within(row).findByRole('status')).toHaveTextContent('Works: おはようございます！');
  });

  it('shows a failed test as an error', async () => {
    const user = userEvent.setup();
    commands.aiProviders.mockResolvedValue(ok([CLAUDE]));
    commands.aiProviderTest.mockResolvedValue({
      status: 'error',
      error: 'the translator answered 401: invalid x-api-key',
    });
    await AppShellWrapper.mount('/settings/ai');
    const row = await screen.findByRole('listitem', { name: 'Claude' });
    await user.click(within(row).getByRole('button', { name: /^Test/ }));
    expect(await within(row).findByRole('alert')).toHaveTextContent('invalid x-api-key');
  });

  it('adds a translator with its key', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/ai');
    expect(await screen.findByText('No translators yet.')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Add translator' }));
    const form = screen.getByRole('form', { name: 'Translator settings' });
    await user.selectOptions(within(form).getByLabelText('Service'), 'openAiCompatible');
    await user.type(within(form).getByLabelText(/^API key/), 'sk-local');
    await user.type(within(form).getByLabelText('Model'), 'qwen3:8b');
    await user.click(within(form).getByRole('button', { name: 'Save' }));

    expect(commands.aiProviderSave).toHaveBeenCalledWith({
      id: null,
      kind: 'openAiCompatible',
      name: 'OpenAI-compatible / local',
      baseUrl: null,
      model: 'qwen3:8b',
      key: 'sk-local',
    });
  });

  it('keeps a saved key unless a new one is typed, and can remove it', async () => {
    const user = userEvent.setup();
    commands.aiProviders.mockResolvedValue(ok([CLAUDE]));
    await AppShellWrapper.mount('/settings/ai');
    const row = await screen.findByRole('listitem', { name: 'Claude' });

    await user.click(within(row).getByRole('button', { name: 'Edit' }));
    let form = screen.getByRole('form', { name: 'Translator settings' });
    expect(within(form).getByLabelText('API key')).toHaveAttribute(
      'placeholder',
      'Saved key ••••a1b2 (leave empty to keep)',
    );
    await user.click(within(form).getByRole('button', { name: 'Save' }));
    expect(commands.aiProviderSave).toHaveBeenLastCalledWith(
      expect.objectContaining({ id: 'p1', key: null }),
    );

    await user.click(
      within(await screen.findByRole('listitem', { name: 'Claude' })).getByRole('button', {
        name: 'Edit',
      }),
    );
    form = screen.getByRole('form', { name: 'Translator settings' });
    await user.click(within(form).getByLabelText('Remove the saved key'));
    await user.click(within(form).getByRole('button', { name: 'Save' }));
    expect(commands.aiProviderSave).toHaveBeenLastCalledWith(
      expect.objectContaining({ id: 'p1', key: '' }),
    );
  });

  it('downloads a model with progress', async () => {
    const user = userEvent.setup();
    commands.aiModels.mockResolvedValue([MODEL]);
    let finish: (value: { status: 'ok'; data: null }) => void = () => {};
    commands.aiModelDownload.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    await AppShellWrapper.mount('/settings/ai');
    const row = await screen.findByRole('listitem', { name: MODEL.name });
    expect(within(row).getByText(/2\.5 MB/)).toBeInTheDocument();

    await user.click(within(row).getByRole('button', { name: `Download ${MODEL.name}` }));
    expect(commands.aiModelDownload).toHaveBeenCalledWith(MODEL.id);
    emitEvent('aiModelEvent', {
      id: MODEL.id,
      state: 'downloading',
      receivedBytes: 1_000_000,
      totalBytes: MODEL.sizeBytes,
      error: null,
    });
    const bar = await within(row).findByRole('progressbar', { name: `Downloading ${MODEL.name}` });
    expect(bar).toHaveAttribute('value', '1000000');

    commands.aiModels.mockResolvedValue([{ ...MODEL, installed: true }]);
    finish(ok(null));
    expect(
      await within(row).findByRole('button', { name: `Delete ${MODEL.name}` }),
    ).toBeInTheDocument();
    expect(within(row).getByText(/Installed/)).toBeInTheDocument();
  });
});
