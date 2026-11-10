import type {
  Lyrics,
  LyricsTranslation,
  LyricsTranslatorOption,
  QueueItem,
  SettingChange,
  Settings,
} from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { LyricsView } from '../player/LyricsView';
import { DEFAULT_SETTINGS, bindingsMock, emitEvent, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import {
  coverage,
  lineTexts,
  pickShown,
  sameLanguage,
  TRANSLATION_DEFAULTS,
  useTranslationChoices,
} from './lyricsTranslation';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const ITEM: QueueItem = {
  id: 'q1',
  title: 'Into the Night',
  artist: 'YOASOBI',
  album: null,
  durationMs: 261_000,
  artworkUrl: null,
  mediaKind: 'audio',
  source: { kind: 'url', url: 'https://example.com/night.flac', headers: {} },
};

const translation = (
  overrides: Partial<LyricsTranslation> & Pick<LyricsTranslation, 'lines'>,
): LyricsTranslation => ({
  language: 'en',
  kind: 'translation',
  origin: 'source',
  by: 'Lyric Lines',
  translator: 'source:Lyric Lines',
  ...overrides,
});

const LYRICS: Lyrics = {
  providerName: 'Lyric Lines',
  synced: [
    { timeMs: 1_000, text: '沈むように' },
    { timeMs: 4_000, text: '♪' },
    { timeMs: 6_000, text: '溶けてゆくように' },
  ],
  plain: null,
  offsetMs: 0,
  edited: false,
  translations: [
    translation({ language: 'zh-Hans', lines: ['宛如沉溺般', '', '宛如慢慢地溶化般'] }),
    translation({
      language: 'ja-Latn',
      kind: 'romanization',
      lines: ['shi zu mu yo u ni', '', 'to ke te yu ku yo u ni'],
    }),
  ],
};

const DEVICE: LyricsTranslatorOption = {
  id: 'device',
  kind: null,
  name: '',
  models: ['opus-mt-mul-en'],
  downloadBytes: 310_361_360,
  supported: true,
};
const CLAUDE: LyricsTranslatorOption = {
  id: 'ai:c1',
  kind: 'claude',
  name: 'Claude',
  models: [],
  downloadBytes: 0,
  supported: true,
};
const DEEPL: LyricsTranslatorOption = {
  ...CLAUDE,
  id: 'ai:d1',
  kind: 'deepl',
  name: 'DeepL',
  supported: false,
};

/** Settings Rust saves and sends back. */
const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
};

const show = () => renderWithProviders(<LyricsView item={ITEM} />);

describe('Lyrics translations', () => {
  beforeEach(() => {
    seedBindings();
    withSettings(DEFAULT_SETTINGS);
    useTranslationChoices.setState({ choices: {} });
    commands.songEditGet.mockResolvedValue({});
  });

  it('matches languages, lines and coverage the way Rust does', () => {
    expect(sameLanguage('zh-Hans', 'zh-CN')).toBe(true);
    expect(sameLanguage('zh-Hans', 'zh-TW')).toBe(false);
    expect(sameLanguage('pt', 'pt-BR')).toBe(true);
    expect(lineTexts({ ...LYRICS, synced: [], plain: 'One\r\n\r\nTwo\n' })).toEqual([
      'One',
      '',
      'Two',
    ]);
    expect(coverage(LYRICS, translation({ lines: ['a', '', ''] }))).toBe(0.5);
    // Another source's translation shows by itself only when it lines up with most lines.
    const other = translation({ origin: 'otherSource', lines: ['a', '', ''] });
    const lyrics = { ...LYRICS, translations: [other] };
    const options = { language: 'en', prefs: TRANSLATION_DEFAULTS, choice: null };
    expect(pickShown(lyrics, options).translation).toBeNull();
    expect(
      pickShown({ ...lyrics, translations: [{ ...other, lines: ['a', '', 'b'] }] }, options)
        .translation,
    ).not.toBeNull();
    // Lyrics kept by an older version have no translations at all.
    expect(pickShown({ ...LYRICS, translations: undefined as never }, options)).toEqual({
      translation: null,
      romanization: null,
    });
  });

  it("shows the source's translation in the listener's language under each line", async () => {
    withSettings({
      ...DEFAULT_SETTINGS,
      lyricsTranslation: { ...TRANSLATION_DEFAULTS, language: 'zh-CN', romanized: true },
    });
    commands.lyricsGet.mockResolvedValue(ok(LYRICS));
    show();
    const view = await screen.findByTestId('lyrics-view');
    const first = await within(view).findByRole('button', { name: /沈むように/ });
    expect(within(first).getByTestId('lyrics-translation')).toHaveTextContent('宛如沉溺般');
    expect(within(first).getByTestId('lyrics-romanized')).toHaveTextContent('shi zu mu yo u ni');
    // The instrumental line has neither.
    const pause = within(view).getByRole('button', { name: '♪' });
    expect(within(pause).queryByTestId('lyrics-translation')).not.toBeInTheDocument();
  });

  it('hides translations in another language until one is picked', async () => {
    const user = userEvent.setup();
    commands.lyricsGet.mockResolvedValue(ok(LYRICS));
    commands.lyricsTranslateOptions.mockResolvedValue(ok([DEVICE, CLAUDE, DEEPL]));
    show();
    const view = await screen.findByTestId('lyrics-view');
    await within(view).findByRole('button', { name: /沈むように/ });
    expect(within(view).queryByTestId('lyrics-translation')).not.toBeInTheDocument();
    expect(within(view).queryByTestId('lyrics-romanized')).not.toBeInTheDocument();

    await user.click(within(view).getByRole('button', { name: 'Translate lyrics' }));
    const menu = await screen.findByTestId('lyrics-translate-menu');
    expect(commands.lyricsTranslateOptions).toHaveBeenCalledWith(
      { title: 'Into the Night', artist: 'YOASOBI', album: null, durationMs: 261_000 },
      'en',
    );
    // Translators that can't: DeepL has no key.
    expect(await within(menu).findByRole('button', { name: /With Claude/ })).toBeEnabled();
    expect(within(menu).getByRole('button', { name: /With DeepL/ })).toBeDisabled();
    expect(within(menu).getByRole('button', { name: /On this device/ })).toHaveTextContent(
      '296 MB download',
    );
    // Lyric Lines's Chinese is one of the song's translations; picking it shows it.
    await user.click(within(menu).getByRole('radio', { name: /From Lyric Lines/ }));
    expect(commands.lyricsTranslationPick).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Into the Night' }),
      'zh-Hans',
      'source:Lyric Lines',
    );
    expect(await within(view).findAllByTestId('lyrics-translation')).toHaveLength(2);
  });

  it('translates with an AI translator and shows the result', async () => {
    const user = userEvent.setup();
    const english = translation({
      language: 'en',
      origin: 'ai',
      by: 'Claude',
      translator: 'ai:c1',
      lines: ['As if sinking', '', 'As if melting away'],
    });
    let translated = false;
    commands.lyricsGet.mockImplementation(async () =>
      ok({
        ...LYRICS,
        translations: translated ? [english, ...(LYRICS.translations ?? [])] : LYRICS.translations,
      }),
    );
    commands.lyricsTranslateOptions.mockResolvedValue(ok([DEVICE, CLAUDE]));
    commands.lyricsTranslate.mockImplementation(async () => {
      translated = true;
      return ok(english);
    });
    show();
    const view = await screen.findByTestId('lyrics-view');
    await user.click(await within(view).findByRole('button', { name: 'Translate lyrics' }));
    const menu = await screen.findByTestId('lyrics-translate-menu');
    await user.click(await within(menu).findByRole('button', { name: /With Claude/ }));
    expect(commands.lyricsTranslate).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Into the Night' }),
      'en',
      'ai:c1',
    );
    const lines = await within(view).findAllByTestId('lyrics-translation');
    expect(lines.map((line) => line.textContent)).toEqual(['As if sinking', 'As if melting away']);
  });

  it('asks before downloading the model, then translates on this device with progress', async () => {
    const user = userEvent.setup();
    commands.lyricsGet.mockResolvedValue(ok(LYRICS));
    commands.lyricsTranslateOptions.mockResolvedValue(ok([DEVICE]));
    commands.aiModels.mockResolvedValue([
      {
        id: 'opus-mt-mul-en',
        name: 'Translate into English (OPUS-MT)',
        kind: 'translate',
        description: '',
        license: 'Apache-2.0',
        licenseUrl: 'https://huggingface.co/Helsinki-NLP/opus-mt-mul-en',
        sourceUrl: 'https://github.com/Helsinki-NLP/Opus-MT',
        sizeBytes: 310_361_360,
        installed: false,
        downloading: null,
      },
    ]);
    let finishDownload: () => void = () => {};
    commands.aiModelDownload.mockImplementation(
      () => new Promise((resolve) => (finishDownload = () => resolve(ok('installed')))),
    );
    let finishTranslation: () => void = () => {};
    commands.lyricsTranslate.mockImplementation(
      () =>
        new Promise((resolve) => {
          finishTranslation = () =>
            resolve(
              ok(
                translation({
                  origin: 'device',
                  by: '',
                  translator: 'device',
                  lines: ['a', '', 'b'],
                }),
              ),
            );
        }),
    );
    show();
    const view = await screen.findByTestId('lyrics-view');
    await user.click(await within(view).findByRole('button', { name: 'Translate lyrics' }));
    const menu = await screen.findByTestId('lyrics-translate-menu');
    await user.click(await within(menu).findByRole('button', { name: /On this device/ }));
    // Nothing downloads before the listener says so.
    expect(commands.aiModelDownload).not.toHaveBeenCalled();
    expect(menu).toHaveTextContent('Download the translation model (296 MB)?');
    await user.click(within(menu).getByRole('button', { name: 'Download' }));
    await waitFor(() => expect(commands.aiModelDownload).toHaveBeenCalledWith('opus-mt-mul-en'));
    emitEvent('aiModelEvent', {
      id: 'opus-mt-mul-en',
      state: 'downloading',
      receivedBytes: 155_180_680,
      totalBytes: 310_361_360,
      error: null,
    });
    expect(
      await screen.findByRole('status', { name: 'Downloading the translation model, 50%' }),
    ).toBeInTheDocument();
    finishDownload();
    await waitFor(() =>
      expect(commands.lyricsTranslate).toHaveBeenCalledWith(
        expect.objectContaining({ title: 'Into the Night' }),
        'en',
        'device',
      ),
    );
    emitEvent('lyricsTranslationEvent', {
      key: 'yoasobi|into the night',
      language: 'en',
      translator: 'device',
      done: 1,
      total: 2,
    });
    expect(
      await screen.findByRole('status', { name: 'Translating line 1 of 2' }),
    ).toHaveTextContent('1/2');
    finishTranslation();
    await waitFor(() => expect(screen.queryByRole('status')).not.toBeInTheDocument());
  });

  it('cancels the translation model’s download, translating nothing', async () => {
    const user = userEvent.setup();
    commands.lyricsGet.mockResolvedValue(ok(LYRICS));
    commands.lyricsTranslateOptions.mockResolvedValue(ok([DEVICE]));
    commands.aiModels.mockResolvedValue([
      {
        id: 'opus-mt-mul-en',
        name: 'Translate into English (OPUS-MT)',
        kind: 'translate',
        description: '',
        license: 'Apache-2.0',
        licenseUrl: 'https://huggingface.co/Helsinki-NLP/opus-mt-mul-en',
        sourceUrl: 'https://github.com/Helsinki-NLP/Opus-MT',
        sizeBytes: 310_361_360,
        installed: false,
        downloading: null,
      },
    ]);
    let stopped: () => void = () => {};
    commands.aiModelDownload.mockImplementation(
      () => new Promise((resolve) => (stopped = () => resolve(ok('cancelled')))),
    );
    commands.aiModelCancel.mockImplementation(async () => {
      stopped();
      return true;
    });
    show();
    const view = await screen.findByTestId('lyrics-view');
    await user.click(await within(view).findByRole('button', { name: 'Translate lyrics' }));
    const menu = await screen.findByTestId('lyrics-translate-menu');
    await user.click(await within(menu).findByRole('button', { name: /On this device/ }));
    await user.click(within(menu).getByRole('button', { name: 'Download' }));
    await waitFor(() => expect(commands.aiModelDownload).toHaveBeenCalledWith('opus-mt-mul-en'));

    await user.click(await screen.findByRole('button', { name: "Cancel the model's download" }));
    expect(commands.aiModelCancel).toHaveBeenCalledWith('opus-mt-mul-en');
    await waitFor(() => expect(screen.queryByRole('status')).not.toBeInTheDocument());
    expect(commands.lyricsTranslate).not.toHaveBeenCalled();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('says calmly when a translator fails, naming it, and tries again', async () => {
    const user = userEvent.setup();
    commands.lyricsGet.mockResolvedValue(ok(LYRICS));
    commands.lyricsTranslateOptions.mockResolvedValue(ok([CLAUDE]));
    commands.lyricsTranslate.mockResolvedValue({
      status: 'error',
      error: 'the translator answered 401: invalid x-api-key',
    });
    show();
    const view = await screen.findByTestId('lyrics-view');
    await user.click(await within(view).findByRole('button', { name: 'Translate lyrics' }));
    await user.click(
      await within(await screen.findByTestId('lyrics-translate-menu')).findByRole('button', {
        name: /With Claude/,
      }),
    );
    const issue = await within(view).findByTestId('source-issue');
    // A refused key reads as the translator refusing access (lib/issue.ts).
    expect(issue).toHaveTextContent('Claude refused access');
    await user.click(within(issue).getByRole('button', { name: 'More info' }));
    expect(issue).toHaveTextContent('invalid x-api-key');
    await user.click(within(issue).getByRole('button', { name: 'Try again' }));
    await waitFor(() => expect(commands.lyricsTranslate).toHaveBeenCalledTimes(2));
  });

  it('turns translations off from the menu', async () => {
    const user = userEvent.setup();
    withSettings({
      ...DEFAULT_SETTINGS,
      lyricsTranslation: { ...TRANSLATION_DEFAULTS, language: 'zh-Hans' },
    });
    commands.lyricsGet.mockResolvedValue(ok(LYRICS));
    show();
    const view = await screen.findByTestId('lyrics-view');
    expect(await within(view).findAllByTestId('lyrics-translation')).toHaveLength(2);
    await user.click(within(view).getByRole('button', { name: 'Translate lyrics' }));
    const menu = await screen.findByTestId('lyrics-translate-menu');
    await user.click(within(menu).getByRole('switch', { name: 'Show translations' }));
    await waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenCalledWith({
        key: 'lyricsTranslation',
        value: expect.objectContaining({ automatic: false }),
      }),
    );
    await waitFor(() =>
      expect(within(view).queryByTestId('lyrics-translation')).not.toBeInTheDocument(),
    );
  });
});
