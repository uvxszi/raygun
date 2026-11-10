import type { AiModel, AiProvider, Captions, SpokenItem } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const CHAPTER = makeItem('Chapter 1', 'Jane Austen');

const BOOK: SpokenItem = {
  kind: 'audiobook',
  podcastId: 'book-1',
  episodeId: 'ch-1',
  show: 'Pride and Prejudice',
  author: 'Jane Austen',
};

const CAPTIONS: Captions = {
  episodeId: 'ch-1',
  cues: [
    { startMs: 0, endMs: 4_000, text: 'It is a truth universally acknowledged,' },
    { startMs: 4_000, endMs: 9_000, text: 'that a single man in possession of a good fortune' },
  ],
  source: 'feed',
  covered: [[0, 9_000]],
  complete: true,
  running: false,
  preparing: false,
  error: null,
  canTranscribe: false,
  engine: 'off',
};

const NONE: Captions = {
  ...CAPTIONS,
  cues: [],
  source: null,
  covered: [],
  complete: false,
};

const setWide = () => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: true,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

const openPanel = async () => {
  const user = userEvent.setup();
  await AppShellWrapper.mount();
  AppShellWrapper.playing([CHAPTER], 0);
  await AppShellWrapper.miniPlayer.title;
  await user.click(
    within(AppShellWrapper.miniPlayer.element).getByRole('button', { name: 'Now playing view' }),
  );
  return { user, panel: screen.getByRole('complementary', { name: 'Now playing' }) };
};

describe('Spoken word in the player', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    setWide();
    commands.spokenItem.mockResolvedValue(ok(BOOK));
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it("shows a chapter's author and captions, not an artist and lyrics", async () => {
    commands.captions.mockResolvedValue(ok(CAPTIONS));
    const { user, panel } = await openPanel();

    const author = await within(panel).findByRole('link', { name: 'Jane Austen' });
    expect(author).toHaveAttribute('href', '/audiobook?id=book-1');
    const captions = await within(panel).findByRole('region', { name: 'Captions' });
    expect(await within(captions).findByText(/universally acknowledged/)).toBeInTheDocument();
    expect(within(captions).getByText('Captions from the feed')).toBeInTheDocument();
    expect(within(panel).queryByRole('region', { name: 'Lyrics' })).not.toBeInTheDocument();
    expect(commands.lyricsGet).not.toHaveBeenCalled();
    // The bar's text button is Captions too.
    expect(
      within(AppShellWrapper.miniPlayer.element).getByRole('button', { name: 'Captions' }),
    ).toBeInTheDocument();

    // A line seeks to where it's said.
    await user.click(within(captions).getByRole('button', { name: /a single man/ }));
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'seek', positionMs: 4_000 });
  });

  it('keeps the card slim without captions and transcribes once set up', async () => {
    commands.captions.mockResolvedValue(ok(NONE));
    const { user, panel } = await openPanel();

    const captions = await within(panel).findByRole('region', { name: 'Captions' });
    expect(await within(captions).findByText('No captions for this chapter.')).toBeInTheDocument();
    expect(within(captions).getByTestId('captions-view').className).not.toContain('h-[26rem]');
    expect(within(captions).getByRole('link', { name: 'Set up transcription' })).toHaveAttribute(
      'href',
      '/settings/ai#transcription',
    );

    // A model is ready now: the card offers to transcribe from where the listener is.
    commands.captions.mockResolvedValue(ok({ ...NONE, canTranscribe: true, engine: 'local' }));
    emitEvent('captionsEvent', 'ch-1');
    await user.click(await within(captions).findByRole('button', { name: 'Transcribe' }));
    expect(commands.captionsTranscribe).toHaveBeenCalledWith('ch-1', expect.any(Number));

    // While it listens, the card says so and can stop.
    commands.captions.mockResolvedValue(
      ok({ ...NONE, canTranscribe: true, engine: 'local', running: true }),
    );
    emitEvent('captionsEvent', 'ch-1');
    expect(await within(captions).findByText(/Listening/)).toBeInTheDocument();
    await user.click(within(captions).getByRole('button', { name: 'Stop' }));
    expect(commands.captionsStop).toHaveBeenCalledWith('ch-1');
  });
});

const SPEECH: AiModel = {
  id: 'whisper-tiny',
  name: 'Speech to text (Whisper tiny)',
  kind: 'speech',
  description: 'Captions on this device.',
  license: 'Apache-2.0',
  licenseUrl: 'https://example.com/license',
  sourceUrl: 'https://example.com',
  sizeBytes: 153_544_121,
  installed: false,
  downloading: null,
};

const GROQ: AiProvider = {
  id: 'groq',
  kind: 'openAiCompatible',
  name: 'Groq',
  baseUrl: 'https://api.groq.com/openai/v1',
  model: null,
  hasKey: true,
  keyHint: 'x9y8',
};

describe('Transcription settings', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('picks who listens: the model on this device or a provider', async () => {
    const user = userEvent.setup();
    let settings = DEFAULT_SETTINGS;
    commands.settingsGet.mockImplementation(async () => ok(settings));
    commands.settingsUpdate.mockImplementation(async (change: { key: string; value: unknown }) => {
      settings = { ...settings, [change.key]: change.value };
      return ok(settings);
    });
    commands.aiModels.mockResolvedValue([SPEECH]);
    commands.aiProviders.mockResolvedValue(ok([GROQ]));
    await AppShellWrapper.mount('/settings/ai#transcription');
    const section = await screen.findByRole('region', { name: 'Transcription' });
    const engine = within(section).getByRole('button', { name: /^Who listens/ });
    await waitFor(() => expect(engine).toBeEnabled());

    await user.click(engine);
    await user.click(await screen.findByRole('radio', { name: /^On this device/ }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'transcription',
      value: expect.objectContaining({ engine: 'local' }),
    });
    expect(
      await within(section).findByRole('button', { name: `Download ${SPEECH.name}` }),
    ).toBeInTheDocument();

    await user.click(within(section).getByRole('button', { name: /^Who listens/ }));
    await user.click(await screen.findByRole('radio', { name: /^Online/ }));
    await user.click(await within(section).findByRole('button', { name: /^Provider/ }));
    await user.click(await screen.findByRole('radio', { name: 'Groq' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'transcription',
      value: expect.objectContaining({ engine: 'online', providerId: 'groq' }),
    });
  });
});
