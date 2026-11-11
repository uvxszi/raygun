import type { AiModel, AiProvider, SettingChange, Settings } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { useCollectionSheets } from '../../collections/collectionSheets';
import { AppShellWrapper } from '../../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS, emitEvent } from '../../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../../test/bindingsMock')).bindingsMock);

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

const GROQ: AiProvider = {
  id: 'groq',
  kind: 'openAiCompatible',
  name: 'Groq',
  baseUrl: 'https://api.groq.com/openai/v1',
  model: null,
  hasKey: true,
  keyHint: 'x9y8',
};

const model = (
  id: string,
  name: string,
  kind: string,
  sizeBytes: number,
  extra: Partial<AiModel> = {},
): AiModel => ({
  id,
  name,
  kind,
  description: '',
  license: 'MIT',
  licenseUrl: 'https://example.com/license',
  sourceUrl: 'https://example.com',
  sizeBytes,
  installed: false,
  downloading: null,
  ...extra,
});

const WHISPER = model('whisper-tiny', 'Speech to text (Whisper tiny)', 'speech', 153_544_121);
const WHISPER_BASE = model('whisper-base', 'Speech to text (Whisper base)', 'speech', 292_886_385);
const OCRS = model('ocrs-latin', 'Text reader (ocrs)', 'ocr', 12_212_656);
const OPUS = model('opus-mt-mul-en', 'Translate into English (OPUS-MT)', 'translate', 312_493_619);
const ANIME4K = model('anime4k-upscale-cnn-x2-s', 'Anime4K 2× (small)', 'upscale', 18_638, {
  engine: 'anime4k',
});
const CUGAN = model('realcugan-up2x-conservative', 'Real-CUGAN 2×', 'upscale', 2_578_409, {
  engine: 'ncnn',
});

const RUNTIME = model(
  'sherpa-onnx-windows-x64',
  'Voice engine (sherpa-onnx)',
  'runtime',
  15_284_969,
  {
    engine: 'sherpa-onnx',
  },
);
const KRISTIN = model(
  'piper-en-us-kristin-medium',
  'Piper: Kristin (US English)',
  'tts',
  20_882_061,
  {
    engine: 'piper',
    installed: true,
  },
);
const ALBA = model('piper-en-gb-alba-medium', 'Piper: Alba (British English)', 'tts', 21_104_326, {
  engine: 'piper',
  installed: true,
});
const THORSTEN = model(
  'piper-de-de-thorsten-medium',
  'Piper: Thorsten (German)',
  'tts',
  20_949_833,
  {
    engine: 'piper',
  },
);
const KOKORO = model(
  'kokoro-multi-lang-int8',
  'Kokoro (49 voices, 8 languages)',
  'tts',
  150_727_420,
  {
    engine: 'kokoro',
  },
);

const voice = (
  modelId: string,
  id: string,
  name: string,
  language: string,
  engine: 'piper' | 'kokoro' = 'piper',
) => ({ key: `${modelId}/${id}`, modelId, engine, name, language, gender: null });

const VOICES = {
  runtimeId: RUNTIME.id,
  voices: [
    voice(KRISTIN.id, 'kristin', 'Kristin', 'en-US'),
    voice(ALBA.id, 'alba', 'Alba', 'en-GB'),
    voice(THORSTEN.id, 'thorsten', 'Thorsten', 'de-DE'),
    voice(KOKORO.id, 'af_heart', 'Heart', 'en-US', 'kokoro'),
  ],
};

/** Settings kept as Rust keeps them: changes answer with the whole new set. */
const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
  commands.settingsUpdateMany.mockImplementation(async (changes: SettingChange[]) => {
    for (const change of changes) current = { ...current, [change.key]: change.value };
    return ok(current);
  });
  return () => current;
};

const region = (name: string) => screen.findByRole('region', { name });

const SECTIONS = [
  [
    'translation',
    'Translation',
    'Comics, books, lyrics and video subtitles, on this device or online',
  ],
  [
    'transcription',
    'Transcription',
    'Captions for podcasts, audiobooks and videos, and lyrics heard from songs',
  ],
  [
    'text-reading',
    'Text reading',
    'Reads words on pictures: song lists in screenshots and comic pages',
  ],
  [
    'read-aloud',
    'Reading aloud',
    "Reads books aloud: with the device's own voices, or voices downloaded here",
  ],
  [
    'providers',
    'Online providers',
    'Claude, OpenAI-compatible services and DeepL; keys stay encrypted on this device',
  ],
  ['models', 'On-device models', 'Run on this device: nothing leaves it'],
] as const;

describe('Settings → AI', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('is a category of the settings list', async () => {
    await AppShellWrapper.mount('/settings');
    const settings = await screen.findByTestId('settings-page');
    expect(within(settings).getByRole('link', { name: 'AI' })).toHaveAttribute(
      'href',
      '/settings/ai',
    );
  });

  it('groups AI by what it does, each group saying in one line what it is for', async () => {
    await AppShellWrapper.mount('/settings/ai');
    const page = await screen.findByTestId('settings-ai-page');
    await region('On-device models');

    const groups = within(page).getAllByRole('region');
    expect(groups.map((group) => group.getAttribute('aria-label'))).toEqual([
      ...SECTIONS.map(([, title]) => title),
      'AI assistants',
    ]);
    for (const [id, title, about] of SECTIONS) {
      const group = within(page).getByRole('region', { name: title });
      expect(group).toHaveAttribute('id', id);
      expect(within(group).getByText(about)).toBeInTheDocument();
    }
  });

  it.each(SECTIONS.map(([id, title]) => [id, title] as const))(
    'opens at #%s, the %s group',
    async (id, title) => {
      await AppShellWrapper.mount(`/settings/ai#${id}`);
      const group = await region(title);
      await waitFor(() => expect(group).toHaveAttribute('data-found'));
    },
  );

  it('links its rows to the places they are used and to the groups they run on', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/ai');
    const translation = await region('Translation');
    const link = (name: RegExp) => within(translation).getByRole('link', { name });

    expect(link(/^Lyrics/)).toHaveAttribute(
      'href',
      '/settings/music#lyricsTranslate.settingsTitle',
    );
    expect(link(/^Video subtitles/)).toHaveAttribute('href', '/library?media=video');
    expect(link(/^On this device/)).toHaveAttribute('href', '/settings/ai#models');
    expect(link(/^Online/)).toHaveAttribute('href', '/settings/ai#providers');

    const transcription = await region('Transcription');
    expect(
      within(transcription).getByRole('link', { name: /^Podcasts and audiobooks/ }),
    ).toHaveAttribute('href', '/library?view=podcasts');
    expect(within(transcription).getByRole('link', { name: /^Videos/ })).toHaveAttribute(
      'href',
      '/library?media=video',
    );

    // The device's translators are in the models group, on the same screen.
    const models = await region('On-device models');
    await user.click(link(/^On this device/));
    await waitFor(() => expect(models).toHaveAttribute('data-found'));
  });

  it('opens lyrics translation where it is set, in the music player settings', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/ai');
    const translation = await region('Translation');

    await user.click(within(translation).getByRole('link', { name: /^Lyrics/ }));
    await screen.findByTestId('settings-music-page');
    const lyrics = await region('Lyrics translation');
    await waitFor(() => expect(lyrics).toHaveAttribute('data-found'));
  });

  it('says what each reader translates with and opens its layers', async () => {
    const user = userEvent.setup();
    withSettings({
      ...DEFAULT_SETTINGS,
      readerAi: {
        comic: [
          { id: 'up', kind: 'upscale', enabled: true, model: '', from: '', to: '' },
          { id: 'tr', kind: 'translate', enabled: true, model: 'ai:p1', from: '', to: 'de' },
        ],
        novel: [{ id: 'tr2', kind: 'translate', enabled: false, model: '', from: '', to: 'en' }],
      },
    });
    commands.aiProviders.mockResolvedValue(ok([CLAUDE]));
    commands.aiModels.mockResolvedValue([OPUS]);
    await AppShellWrapper.mount('/settings/ai');
    const translation = await region('Translation');

    expect(await within(translation).findByRole('button', { name: /^Comics/ })).toHaveTextContent(
      'Claude · into Deutsch',
    );
    expect(within(translation).getByRole('button', { name: /^Books/ })).toHaveTextContent('Off');
    expect(within(translation).getByRole('link', { name: /^On this device/ })).toHaveTextContent(
      'No models installed',
    );
    expect(within(translation).getByRole('link', { name: /^Online/ })).toHaveTextContent('Claude');

    await user.click(within(translation).getByRole('button', { name: /^Comics/ }));
    const layers = await screen.findByRole('dialog', { name: 'AI layers' });
    expect(within(layers).getByRole('listitem', { name: 'Translate' })).toBeInTheDocument();
    expect(
      within(layers).getByRole('link', { name: 'More models in Settings → AI' }),
    ).toHaveAttribute('href', '/settings/ai#models');
  });

  it('picks the model on this device and downloads it in place', async () => {
    const user = userEvent.setup();
    const saved = withSettings({
      ...DEFAULT_SETTINGS,
      transcription: { ...DEFAULT_SETTINGS.transcription, engine: 'local' },
    });
    commands.aiModels.mockResolvedValue([WHISPER, WHISPER_BASE]);
    commands.aiModelDownload.mockReturnValue(new Promise(() => {}));
    await AppShellWrapper.mount('/settings/ai');
    const transcription = await region('Transcription');

    await user.click(await within(transcription).findByRole('button', { name: /^Model/ }));
    await user.click(
      await screen.findByRole('radio', { name: /^Speech to text \(Whisper base\)/ }),
    );
    await waitFor(() => expect(saved().transcription.localModel).toBe('whisper-base'));

    const download = await within(transcription).findByRole('button', {
      name: `Download ${WHISPER_BASE.name}`,
    });
    expect(download).toHaveTextContent('279 MB');
    await user.click(download);
    expect(commands.aiModelDownload).toHaveBeenCalledWith('whisper-base');
    emitEvent('aiModelEvent', {
      id: 'whisper-base',
      state: 'downloading',
      receivedBytes: 146_443_193,
      totalBytes: WHISPER_BASE.sizeBytes,
      error: null,
    });
    expect(await within(transcription).findByText('Downloading · 50%')).toBeInTheDocument();
    expect(
      within(transcription).getByRole('progressbar', { name: `Downloading ${WHISPER_BASE.name}` }),
    ).toHaveAttribute('value', '146443193');
  });

  it('sets up an online provider for transcription in a sheet', async () => {
    const user = userEvent.setup();
    const saved = withSettings({
      ...DEFAULT_SETTINGS,
      transcription: { ...DEFAULT_SETTINGS.transcription, engine: 'online' },
    });
    commands.aiProviders.mockResolvedValue(ok([CLAUDE]));
    await AppShellWrapper.mount('/settings/ai');
    const transcription = await region('Transcription');

    // Claude can't hear: a Whisper server (or an OpenAI-compatible provider) is added here.
    await user.click(
      await within(transcription).findByRole('button', {
        name: /^Add a Whisper server/,
      }),
    );
    const form = await screen.findByRole('form', { name: 'Provider settings' });
    expect(within(form).getByLabelText('Service')).toHaveValue('whisperServer');
    expect(form).toHaveTextContent('It only transcribes.');
    await user.click(within(form).getByRole('button', { name: 'Cancel' }));

    await user.click(within(transcription).getByRole('button', { name: /^Online model/ }));
    const field = await screen.findByRole('textbox', { name: 'Online model' });
    await user.clear(field);
    await user.type(field, 'whisper-large-v3{Enter}');
    await waitFor(() => expect(saved().transcription.onlineModel).toBe('whisper-large-v3'));
  });

  it('reads text with Windows, or downloads the text reader', async () => {
    const user = userEvent.setup();
    commands.readerAiSupport.mockResolvedValue({
      textReaderBuiltIn: true,
      textReaderModel: 'ocrs-latin',
    });
    commands.aiModels.mockResolvedValue([OCRS]);
    await AppShellWrapper.mount('/settings/ai');
    const reading = await region('Text reading');
    expect(await within(reading).findByText('Built into Windows')).toBeInTheDocument();
    expect(within(reading).queryByRole('button', { name: /^Download/ })).not.toBeInTheDocument();

    // Import songs reads screenshots: it opens from here.
    await user.click(within(reading).getByRole('button', { name: /^Import songs/ }));
    expect(useCollectionSheets.getState().importing).toBe(true);
    useCollectionSheets.getState().close();
  });

  it('downloads the text reader where the system has none', async () => {
    const user = userEvent.setup();
    commands.readerAiSupport.mockResolvedValue({
      textReaderBuiltIn: false,
      textReaderModel: 'ocrs-latin',
    });
    commands.aiModels.mockResolvedValue([OCRS]);
    await AppShellWrapper.mount('/settings/ai');
    const reading = await region('Text reading');

    expect(await within(reading).findByText('Text reader (ocrs)')).toBeInTheDocument();
    await user.click(within(reading).getByRole('button', { name: `Download ${OCRS.name}` }));
    expect(commands.aiModelDownload).toHaveBeenCalledWith('ocrs-latin');
  });

  it('reads aloud with downloaded voices: picks the engine, a voice per language, downloads', async () => {
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    commands.ttsVoices.mockResolvedValue(VOICES);
    commands.aiModels.mockResolvedValue([RUNTIME, KRISTIN, ALBA, THORSTEN, KOKORO]);
    commands.aiModelDownload.mockReturnValue(new Promise(() => {}));
    await AppShellWrapper.mount('/settings/ai#read-aloud');
    const group = await region('Reading aloud');

    await user.click(await within(group).findByRole('button', { name: /^Voices/ }));
    await user.click(await screen.findByRole('radio', { name: /^Piper/ }));
    await waitFor(() => expect(saved().readAloud.engine).toBe('piper'));
    // The engine that runs voices comes with the first one.
    expect(
      await within(group).findByText('Downloaded with the first voice (15 MB)'),
    ).toBeInTheDocument();

    // The app's language first: its voices, the first downloaded one reading it.
    const english = await within(group).findByRole('list', { name: 'English voices' });
    const kristin = within(english).getByRole('radio', { name: 'Read English with Kristin' });
    expect(kristin).toBeChecked();
    await user.click(within(english).getByRole('radio', { name: 'Read English with Alba' }));
    await waitFor(() =>
      expect(saved().readAloud.voices).toEqual({ en: 'piper-en-gb-alba-medium/alba' }),
    );
    await user.click(within(english).getByRole('button', { name: 'Hear Kristin' }));
    expect(commands.ttsPreview).toHaveBeenCalledWith('piper-en-us-kristin-medium/kristin', 'en-US');

    // Another language: its voices download in place, and can't be picked before.
    await user.click(within(group).getByRole('button', { name: /^Language/ }));
    await user.click(await screen.findByRole('radio', { name: 'German' }));
    const german = await within(group).findByRole('list', { name: 'German voices' });
    expect(within(german).getByRole('radio', { name: 'Read German with Thorsten' })).toBeDisabled();
    await user.click(within(german).getByRole('button', { name: `Download ${THORSTEN.name}` }));
    expect(commands.aiModelDownload).toHaveBeenCalledWith(THORSTEN.id);

    // Kokoro's voices are one download.
    await user.click(within(group).getByRole('button', { name: /^Voices/ }));
    await user.click(await screen.findByRole('radio', { name: /^Kokoro/ }));
    await waitFor(() => expect(saved().readAloud.engine).toBe('kokoro'));
    expect(
      await within(group).findByRole('button', { name: `Download ${KOKORO.name}` }),
    ).toBeInTheDocument();
    expect(
      within(group).queryByRole('button', { name: `Download ${THORSTEN.name}` }),
    ).not.toBeInTheDocument();
  });

  it('reads aloud with the device’s voices at a pitch, with a preview', async () => {
    const user = userEvent.setup();
    withSettings({ ...DEFAULT_SETTINGS, readAloud: { engine: 'system', voices: {}, pitch: 12 } });
    commands.ttsVoices.mockResolvedValue({ runtimeId: null, voices: [] });
    await AppShellWrapper.mount('/settings/ai#read-aloud');
    const group = await region('Reading aloud');

    expect(
      await within(group).findByText("Downloaded voices don't run on this device yet"),
    ).toBeInTheDocument();
    expect(within(group).getByRole('slider', { name: 'Pitch' })).toHaveAttribute(
      'aria-valuetext',
      '1.2×',
    );
    expect(within(group).getByRole('slider', { name: 'Reading-aloud speed' })).toBeInTheDocument();
    await user.click(within(group).getByRole('button', { name: /^Preview/ }));
    expect(commands.ttsPreview).toHaveBeenCalledWith(null, 'en');
    // Only the device's voices are offered here.
    await user.click(within(group).getByRole('button', { name: /^Voices/ }));
    expect(await screen.findByRole('radio', { name: /^Device voices/ })).toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: /^Piper/ })).not.toBeInTheDocument();
  });

  it('lists providers with their key hints and tests one', async () => {
    const user = userEvent.setup();
    commands.aiProviders.mockResolvedValue(ok([CLAUDE]));
    commands.aiProviderTest.mockResolvedValue(ok('おはようございます！'));
    await AppShellWrapper.mount('/settings/ai');

    const providers = await region('Online providers');
    const row = await within(providers).findByRole('listitem', { name: 'Claude' });
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
    const problem = await within(row).findByRole('alert');
    expect(problem).toHaveTextContent("Claude didn't translate");
    await user.click(within(problem).getByRole('button', { name: 'More info' }));
    expect(problem).toHaveTextContent('invalid x-api-key');
  });

  it('adds a provider with its key in a sheet', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/ai');
    const providers = await region('Online providers');
    expect(await within(providers).findByText('No providers yet.')).toBeInTheDocument();

    await user.click(within(providers).getByRole('button', { name: 'Add provider' }));
    const form = screen.getByRole('form', { name: 'Provider settings' });
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
    let form = screen.getByRole('form', { name: 'Provider settings' });
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
    form = screen.getByRole('form', { name: 'Provider settings' });
    await user.click(within(form).getByLabelText('Remove the saved key'));
    await user.click(within(form).getByRole('button', { name: 'Save' }));
    expect(commands.aiProviderSave).toHaveBeenLastCalledWith(
      expect.objectContaining({ id: 'p1', key: '' }),
    );
  });

  /** The model rows shown, in order. */
  const modelRows = (models: HTMLElement) =>
    within(models)
      .getAllByRole('listitem')
      .map((row) => row.getAttribute('aria-label'))
      .filter(Boolean);

  it('lists the models by name, with their uses, sizes and the space they take', async () => {
    commands.aiModels.mockResolvedValue([
      CUGAN,
      ANIME4K,
      OCRS,
      { ...WHISPER, installed: true },
      { ...OPUS, installed: true },
    ]);
    await AppShellWrapper.mount('/settings/ai');
    const models = await region('On-device models');

    expect(await within(models).findByText('444 MB · 2 models')).toBeInTheDocument();
    // ncnn's upscalers don't run in this version: they're listed only to remove them.
    expect(modelRows(models)).toEqual([ANIME4K.name, WHISPER.name, OCRS.name, OPUS.name]);
    expect(
      within(within(models).getByRole('listitem', { name: OPUS.name })).getByText(
        'Translation · 298 MB · Installed',
      ),
    ).toBeInTheDocument();
    expect(
      within(within(models).getByRole('listitem', { name: ANIME4K.name })).getByText(
        'Comic reader · 18 KB',
      ),
    ).toBeInTheDocument();
  });

  it('narrows the models by use with pills, and sorts them', async () => {
    const user = userEvent.setup();
    commands.aiModels.mockResolvedValue([
      { ...CUGAN, installed: true },
      ANIME4K,
      OCRS,
      WHISPER,
      { ...WHISPER_BASE, installed: true },
      OPUS,
    ]);
    await AppShellWrapper.mount('/settings/ai');
    const models = await region('On-device models');
    await within(models).findByRole('listitem', { name: OPUS.name });
    const pills = within(models).getByRole('group', { name: 'Show models' });
    expect(
      within(pills)
        .getAllByRole('button')
        .map((pill) => pill.textContent),
    ).toEqual([
      'All',
      'Translation',
      'Transcription',
      'Text reading',
      'Reading aloud',
      'Upscaling',
      'Downloaded',
    ]);
    expect(within(pills).getByRole('button', { name: 'All' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    const pill = (name: string) => user.click(within(pills).getByRole('button', { name }));

    await pill('Translation');
    expect(within(pills).getByRole('button', { name: 'Translation' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(modelRows(models)).toEqual([OPUS.name]);
    await pill('Transcription');
    expect(modelRows(models)).toEqual([WHISPER_BASE.name, WHISPER.name]);
    await pill('Text reading');
    expect(modelRows(models)).toEqual([OCRS.name]);
    // The readers' upscalers, an ncnn one too while it's downloaded (to remove it).
    await pill('Upscaling');
    expect(modelRows(models)).toEqual([ANIME4K.name, CUGAN.name]);
    await pill('Downloaded');
    expect(modelRows(models)).toEqual([CUGAN.name, WHISPER_BASE.name]);

    await pill('All');
    const sort = within(models).getByRole('combobox', { name: 'Sort by' });
    expect(sort).toHaveValue('name');
    await user.selectOptions(sort, 'Size');
    expect(modelRows(models)).toEqual([
      OPUS.name,
      WHISPER_BASE.name,
      WHISPER.name,
      OCRS.name,
      CUGAN.name,
      ANIME4K.name,
    ]);
    await user.selectOptions(sort, 'Downloaded first');
    expect(modelRows(models)).toEqual([
      CUGAN.name,
      WHISPER_BASE.name,
      ANIME4K.name,
      WHISPER.name,
      OCRS.name,
      OPUS.name,
    ]);
  });

  it('says when no model is downloaded yet', async () => {
    const user = userEvent.setup();
    commands.aiModels.mockResolvedValue([OCRS, OPUS]);
    await AppShellWrapper.mount('/settings/ai');
    const models = await region('On-device models');
    await within(models).findByRole('listitem', { name: OPUS.name });
    await user.click(
      within(within(models).getByRole('group', { name: 'Show models' })).getByRole('button', {
        name: 'Downloaded',
      }),
    );
    expect(within(models).getByText('No models downloaded yet')).toBeInTheDocument();
    expect(modelRows(models)).toEqual([]);
  });

  it('cancels a download from its row, leaving the model as it was', async () => {
    const user = userEvent.setup();
    commands.aiModels.mockResolvedValue([OPUS]);
    let finish: (value: { status: 'ok'; data: 'cancelled' }) => void = () => {};
    commands.aiModelDownload.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    await AppShellWrapper.mount('/settings/ai');
    const models = await region('On-device models');
    const row = await within(models).findByRole('listitem', { name: OPUS.name });

    await user.click(within(row).getByRole('button', { name: `Download ${OPUS.name}` }));
    emitEvent('aiModelEvent', {
      id: OPUS.id,
      state: 'downloading',
      receivedBytes: 31_249_362,
      totalBytes: OPUS.sizeBytes,
      error: null,
    });
    expect(
      await within(row).findByText('Translation · 298 MB · Downloading · 10%'),
    ).toBeInTheDocument();
    expect(within(row).queryByRole('button', { name: `Download ${OPUS.name}` })).toBeNull();

    await user.click(within(row).getByRole('button', { name: `Cancel downloading ${OPUS.name}` }));
    expect(commands.aiModelCancel).toHaveBeenCalledWith(OPUS.id);
    finish(ok('cancelled'));
    emitEvent('aiModelEvent', {
      id: OPUS.id,
      state: 'cancelled',
      receivedBytes: 0,
      totalBytes: OPUS.sizeBytes,
      error: null,
    });
    // Back as it was: Download again, no bar and nothing said to have failed.
    expect(
      await within(row).findByRole('button', { name: `Download ${OPUS.name}` }),
    ).toBeInTheDocument();
    expect(within(row).getByText('Translation · 298 MB')).toBeInTheDocument();
    expect(within(row).queryByRole('progressbar')).toBeNull();
    expect(within(row).queryByRole('alert')).toBeNull();
  });

  it('cancels the speech model downloading in Transcription', async () => {
    const user = userEvent.setup();
    withSettings({
      ...DEFAULT_SETTINGS,
      transcription: { ...DEFAULT_SETTINGS.transcription, engine: 'local' },
    });
    commands.aiModels.mockResolvedValue([{ ...WHISPER, downloading: 15_354_413 }]);
    await AppShellWrapper.mount('/settings/ai');
    const transcription = await region('Transcription');
    expect(await within(transcription).findByText('Downloading · 10%')).toBeInTheDocument();

    // Rust's list, once it stopped.
    commands.aiModels.mockResolvedValue([WHISPER]);
    await user.click(
      within(transcription).getByRole('button', { name: `Cancel downloading ${WHISPER.name}` }),
    );
    expect(commands.aiModelCancel).toHaveBeenCalledWith(WHISPER.id);
    expect(
      await within(transcription).findByRole('button', { name: `Download ${WHISPER.name}` }),
    ).toBeInTheDocument();
  });

  it("cancels a layer's model downloading in the reader's AI layers", async () => {
    const user = userEvent.setup();
    withSettings({
      ...DEFAULT_SETTINGS,
      readerAi: {
        comic: [{ id: 'up', kind: 'upscale', enabled: true, model: '', from: '', to: '' }],
        novel: [],
      },
    });
    commands.aiModels.mockResolvedValue([ANIME4K]);
    commands.aiModelDownload.mockReturnValue(new Promise(() => {}));
    await AppShellWrapper.mount('/settings/ai');
    const reading = await region('Text reading');
    await user.click(within(reading).getByRole('button', { name: /^Comic translation/ }));
    const layers = await screen.findByRole('dialog', { name: 'AI layers' });
    const layer = within(layers).getByRole('listitem', { name: 'Upscale' });

    await user.click(await within(layer).findByRole('button', { name: 'Download' }));
    expect(commands.aiModelDownload).toHaveBeenCalledWith(ANIME4K.id);
    emitEvent('aiModelEvent', {
      id: ANIME4K.id,
      state: 'downloading',
      receivedBytes: 9_319,
      totalBytes: ANIME4K.sizeBytes,
      error: null,
    });
    await waitFor(() => expect(within(layer).getByRole('status')).toHaveTextContent('50%'));
    await user.click(
      within(layer).getByRole('button', { name: `Cancel downloading ${ANIME4K.name}` }),
    );
    expect(commands.aiModelCancel).toHaveBeenCalledWith(ANIME4K.id);
    expect(await within(layer).findByRole('button', { name: 'Download' })).toBeInTheDocument();
  });

  it('downloads a model with progress and deletes it', async () => {
    const user = userEvent.setup();
    commands.aiModels.mockResolvedValue([ANIME4K, { ...CUGAN, installed: true }]);
    let finish: (value: { status: 'ok'; data: 'installed' }) => void = () => {};
    commands.aiModelDownload.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    await AppShellWrapper.mount('/settings/ai');
    const models = await region('On-device models');
    expect(
      within(await within(models).findByRole('listitem', { name: CUGAN.name })).getByText(
        'Not used in this version · 2.5 MB · Installed',
      ),
    ).toBeInTheDocument();
    const row = within(models).getByRole('listitem', { name: ANIME4K.name });

    await user.click(within(row).getByRole('button', { name: `Download ${ANIME4K.name}` }));
    expect(commands.aiModelDownload).toHaveBeenCalledWith(ANIME4K.id);
    emitEvent('aiModelEvent', {
      id: ANIME4K.id,
      state: 'downloading',
      receivedBytes: 9_319,
      totalBytes: ANIME4K.sizeBytes,
      error: null,
    });
    const bar = await within(row).findByRole('progressbar', {
      name: `Downloading ${ANIME4K.name}`,
    });
    expect(bar).toHaveAttribute('value', '9319');
    expect(within(row).getByText('Comic reader · 18 KB · Downloading · 50%')).toBeInTheDocument();

    commands.aiModels.mockResolvedValue([{ ...ANIME4K, installed: true }, CUGAN]);
    finish(ok('installed'));
    await user.click(await within(row).findByRole('button', { name: `Delete ${ANIME4K.name}` }));
    expect(commands.aiModelDelete).toHaveBeenCalledWith(ANIME4K.id);
  });

  it('resets transcription and the readers’ layers, keeping providers and models', async () => {
    const user = userEvent.setup();
    const saved = withSettings({
      ...DEFAULT_SETTINGS,
      transcription: { ...DEFAULT_SETTINGS.transcription, engine: 'online', providerId: 'groq' },
      readerAi: {
        comic: [{ id: 'tr', kind: 'translate', enabled: true, model: '', from: '', to: 'de' }],
        novel: [],
      },
    });
    commands.settingsDefaults.mockResolvedValue({
      ...DEFAULT_SETTINGS,
      readerAi: { comic: [], novel: [] },
    });
    commands.aiProviders.mockResolvedValue(ok([GROQ]));
    await AppShellWrapper.mount('/settings/ai');
    await region('Transcription');

    await user.click(await screen.findByRole('button', { name: 'More options' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Reset to defaults' }));
    await user.click(await screen.findByRole('button', { name: 'Reset' }));
    await waitFor(() => expect(saved().transcription.engine).toBe('off'));
    expect(saved().readerAi).toEqual({ comic: [], novel: [] });
    expect(commands.aiProviderDelete).not.toHaveBeenCalled();
    expect(commands.aiModelDelete).not.toHaveBeenCalled();
  });
});
