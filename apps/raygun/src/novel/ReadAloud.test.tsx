import type {
  AiModel,
  NovelChapter,
  SettingChange,
  Settings,
  TtsState,
  TtsUpdate,
  TtsVoice,
} from '@raygun/bindings';
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const user = () => userEvent.setup();

const PARAGRAPH = 'It was late. Who was there? Nobody answered.';

const CHAPTER: NovelChapter = {
  titleId: 't1',
  title: 'Frankenstein',
  chapter: {
    id: 'c1',
    name: 'Letter 1',
    number: null,
    volume: null,
    scanlator: null,
    dateUpload: null,
    read: false,
    lastPage: null,
    totalPages: null,
    download: null,
  },
  blocks: [
    { kind: 'heading', text: 'Chapter IV' },
    { kind: 'paragraph', text: PARAGRAPH },
    { kind: 'paragraph', text: 'The end came.' },
  ],
  previousChapterId: null,
  nextChapterId: null,
};

const update = (state: TtsState, paragraph: number, extra: Partial<TtsUpdate> = {}): TtsUpdate => ({
  state,
  chapterId: 'c1',
  paragraph,
  start: 0,
  end: 0,
  notice: null,
  ...extra,
});

const MODEL: AiModel = {
  id: 'kokoro-multi',
  name: 'Kokoro',
  kind: 'tts',
  description: 'Voices.',
  license: 'Apache-2.0',
  licenseUrl: 'https://example.com/license',
  sourceUrl: 'https://example.com',
  sizeBytes: 300_000_000,
  installed: true,
  downloading: null,
  engine: 'kokoro',
};

const voice = (id: string, name: string, language: string): TtsVoice => ({
  key: `kokoro-multi/${id}`,
  modelId: 'kokoro-multi',
  engine: 'kokoro',
  name,
  language,
  gender: 'female',
});

/**
 * Settings that keep what's saved, as Rust does; choosing one of `voices` reads its language with
 * it, as Rust does once it's downloaded.
 */
const withSettings = (settings: Settings, voices: TtsVoice[] = []) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
  commands.ttsUseVoice.mockImplementation(async (key: string | null) => {
    const chosen = voices.find((candidate) => candidate.key === key);
    const prefs = current.readAloud;
    current = {
      ...current,
      readAloud: chosen
        ? {
            ...prefs,
            engine: chosen.engine,
            voices: { ...prefs.voices, [chosen.language.slice(0, 2)]: chosen.key },
          }
        : { ...prefs, engine: 'system' },
    };
    return ok('installed' as const);
  });
  return () => current;
};

/** Without word lookups, holding the text reads aloud from there. */
const NO_LOOKUPS: Settings = {
  ...DEFAULT_SETTINGS,
  dictionary: { enabled: false, order: [], off: [], language: '' },
};

/** The text node holding `text` in `element`, and where `text` starts in it. */
const textNode = (element: HTMLElement, text: string) => {
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const at = node.textContent?.indexOf(text) ?? -1;
    if (at >= 0) return { node, at };
  }
  throw new Error(`no text "${text}"`);
};

/** jsdom lays nothing out: taps land where a test puts the caret, `into` characters into `text`. */
const tapLandsOn = (element: HTMLElement, text: string, into = 1) => {
  const { node, at } = textNode(element, text);
  Object.assign(document, {
    caretRangeFromPoint: () => {
      const range = document.createRange();
      range.setStart(node, at + into);
      return range;
    },
  });
};

const openReader = async () => {
  commands.ttsAvailable.mockResolvedValue(ok(true));
  commands.novelChapter.mockResolvedValue(ok(CHAPTER));
  await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
  const reader = await screen.findByTestId('novel-reader');
  await within(reader).findByRole('button', { name: 'Read aloud' });
  return reader;
};

describe('Reading aloud in the book reader', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    Element.prototype.scrollIntoView = vi.fn();
    document.documentElement.dataset.motion = 'off';
  });

  afterEach(() => {
    delete (document as { caretRangeFromPoint?: unknown }).caretRangeFromPoint;
    delete document.documentElement.dataset.motion;
  });

  it('starts at the sentence held down, and a tap while reading goes to the sentence tapped', async () => {
    withSettings(NO_LOOKUPS);
    const reader = await openReader();
    const paragraph = within(reader).getByText(PARAGRAPH);

    // Held down in "Who was there?": from that sentence, not the paragraph's start.
    tapLandsOn(paragraph, 'Who', 2);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    fireEvent.pointerDown(paragraph, { clientX: 40, clientY: 40 });
    await act(() => vi.advanceTimersByTimeAsync(600));
    fireEvent.pointerUp(paragraph);
    vi.useRealTimers();
    // The click that ends the press does nothing more.
    fireEvent.click(paragraph);
    expect(commands.ttsSpeak).toHaveBeenCalledTimes(1);
    expect(commands.ttsSpeak).toHaveBeenLastCalledWith(
      expect.objectContaining({ from: 1, offset: 15, resume: false, titleId: 't1' }),
    );
    // Titles say so, for their numerals.
    expect(commands.ttsSpeak).toHaveBeenLastCalledWith(
      expect.objectContaining({
        paragraphs: expect.arrayContaining([{ id: 0, text: 'Chapter IV', heading: true }]),
      }),
    );

    emitEvent('ttsEvent', update('speaking', 1, { start: 13, end: 27 }));
    expect(within(reader).getByTestId('spoken-sentence')).toHaveTextContent('Who was there?');
    commands.ttsSpeak.mockClear();

    // A tap on another sentence reads from it ("Nobody" starts at 28).
    const marked = within(reader).getByText('It was late.', { exact: false });
    tapLandsOn(marked, 'Nobody', 3);
    fireEvent.click(marked);
    expect(commands.ttsSpeak).toHaveBeenLastCalledWith(
      expect.objectContaining({ from: 1, offset: 31, resume: false }),
    );

    // A tap on the sentence being read shows or hides the bars instead.
    emitEvent('ttsEvent', update('speaking', 1, { start: 28, end: 44 }));
    commands.ttsSpeak.mockClear();
    const sentence = within(reader).getByTestId('spoken-sentence');
    expect(sentence).toHaveTextContent('Nobody answered.');
    tapLandsOn(sentence, 'answered', 1);
    expect(within(reader).getByRole('button', { name: 'Text settings' })).toBeInTheDocument();
    fireEvent.click(sentence);
    expect(commands.ttsSpeak).not.toHaveBeenCalled();
    expect(within(reader).queryByRole('button', { name: 'Text settings' })).toBeNull();
  });

  it('turns pages from the sides while reading in paged mode, and reads from a sentence tapped in the middle', async () => {
    withSettings({
      ...NO_LOOKUPS,
      novelReader: { ...DEFAULT_SETTINGS.novelReader, paged: true },
    });
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.novelChapter.mockResolvedValue(ok(CHAPTER));
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const pages = await screen.findByTestId('novel-pages');
    const reader = screen.getByTestId('novel-reader');
    await user().click(await within(reader).findByRole('button', { name: 'Read aloud' }));
    emitEvent('ttsEvent', update('speaking', 1, { start: 0, end: 12 }));
    commands.ttsSpeak.mockClear();
    vi.spyOn(pages, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      width: 300,
      top: 0,
      height: 600,
    } as DOMRect);
    const paragraph = within(reader).getByText('The end came.');
    tapLandsOn(paragraph, 'end', 0);

    // The left third goes back a page (none here): nothing is read from there.
    fireEvent.click(paragraph, { clientX: 20 });
    expect(commands.ttsSpeak).not.toHaveBeenCalled();
    // Moving by hand holds following, with a way back.
    expect(
      await within(reader).findByRole('button', { name: 'Back to reading' }),
    ).toBeInTheDocument();
    fireEvent.click(paragraph, { clientX: 150 });
    expect(commands.ttsSpeak).toHaveBeenLastCalledWith(
      expect.objectContaining({ from: 2, offset: 4 }),
    );
  });

  it('follows the sentence read, and waits after a scroll by hand', async () => {
    withSettings(DEFAULT_SETTINGS);
    const reader = await openReader();
    await user().click(within(reader).getByRole('button', { name: 'Read aloud' }));
    const main = reader.closest('main')!;
    Object.defineProperty(main, 'scrollHeight', { configurable: true, value: 5000 });
    Object.defineProperty(main, 'clientHeight', { configurable: true, value: 800 });
    vi.spyOn(main, 'getBoundingClientRect').mockReturnValue({
      top: 0,
      bottom: 800,
      height: 800,
      left: 0,
      width: 400,
    } as DOMRect);
    // Every sentence is far down the column until the column moves.
    const below = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(() => {
        const top = 1200 - main.scrollTop;
        return { top, bottom: top + 40, height: 40, left: 0, width: 300 } as DOMRect;
      });

    emitEvent('ttsEvent', update('speaking', 1, { start: 0, end: 12 }));
    // To a little above the middle: 1200 − 38 % of 800.
    await waitFor(() => expect(main.scrollTop).toBe(896));

    // Scrolled by hand: the next sentence doesn't move the column.
    main.scrollTop = 0;
    fireEvent.wheel(main, { deltaY: -400 });
    emitEvent('ttsEvent', update('speaking', 1, { start: 13, end: 27 }));
    await act(async () => {});
    expect(main.scrollTop).toBe(0);
    await user().click(await within(reader).findByRole('button', { name: 'Back to reading' }));
    await waitFor(() => expect(main.scrollTop).toBe(896));
    expect(within(reader).queryByRole('button', { name: 'Back to reading' })).toBeNull();
    below.mockRestore();
  });

  it('sets the voice, speed, pitch, volume, language and numerals in the reader', async () => {
    const voices = [
      voice('af_heart', 'Heart', 'en-US'),
      voice('bf_emma', 'Emma', 'en-GB'),
      voice('ef_dora', 'Dora', 'es'),
    ];
    const saved = withSettings(
      {
        ...DEFAULT_SETTINGS,
        readAloud: { engine: 'kokoro', voices: {}, pitch: 10, volume: 100 },
      },
      voices,
    );
    commands.ttsVoices.mockResolvedValue({ runtimeId: 'sherpa-onnx', voices });
    commands.aiModels.mockResolvedValue([MODEL]);
    commands.ttsLanguage.mockResolvedValue({ chosen: null, book: 'en' });
    const reader = await openReader();

    await user().click(within(reader).getByRole('button', { name: 'Text settings' }));
    const panel = within(reader).getByRole('dialog', { name: 'Text settings' });
    await user().click(within(panel).getByRole('tab', { name: 'Read aloud' }));
    const options = within(panel).getByRole('list', { name: 'Reading aloud' });

    // The voice reading the book's language, among its downloaded voices and the device's.
    const voiceRow = await within(options).findByRole('button', { name: /^Voice/ });
    await waitFor(() => expect(voiceRow).toHaveTextContent('Heart'));
    await user().click(voiceRow);
    const choices = await screen.findByRole('radiogroup', { name: 'Voice' });
    expect(
      within(choices)
        .getAllByRole('radio')
        .map((radio) => radio.textContent),
    ).toEqual(["Device's voice", 'HeartKokoro · American English', 'EmmaKokoro · British English']);
    await user().click(within(choices).getByRole('radio', { name: /^Emma/ }));
    expect(commands.ttsUseVoice).toHaveBeenCalledWith('kokoro-multi/bf_emma');
    await waitFor(() =>
      expect(saved().readAloud).toMatchObject({
        engine: 'kokoro',
        voices: { en: 'kokoro-multi/bf_emma' },
      }),
    );
    await waitFor(() => expect(voiceRow).toHaveTextContent('Emma'));

    // Volume and pitch save when let go; Rust applies them to the reading.
    const volume = within(options).getByRole('slider', { name: 'Volume' });
    fireEvent.change(volume, { target: { value: '60' } });
    await waitFor(() => expect(saved().readAloud.volume).toBe(60));
    const pitch = within(options).getByRole('slider', { name: 'Pitch' });
    fireEvent.change(pitch, { target: { value: '12' } });
    await waitFor(() => expect(saved().readAloud.pitch).toBe(12));

    // The book's language is Auto; another is chosen for this book.
    const languageRow = within(options).getByRole('button', { name: /^Reading language/ });
    expect(languageRow).toHaveTextContent('Auto (English)');
    await user().click(languageRow);
    await user().click(await screen.findByRole('radio', { name: 'Spanish' }));
    expect(commands.ttsSetLanguage).toHaveBeenCalledWith('t1', 'es');

    // Numerals: titles by default; the rest goes with it.
    const titles = within(options).getByRole('switch', {
      name: 'Read Roman numerals in titles as numbers',
    });
    expect(titles).toBeChecked();
    const everywhere = within(options).getByRole('switch', {
      name: "Also where they're clearly numerals",
    });
    expect(everywhere).not.toBeChecked();
    await user().click(everywhere);
    await waitFor(() => expect(saved().readAloud.romanEverywhere).toBe(true));
    await user().click(titles);
    await waitFor(() => expect(saved().readAloud.romanTitles).toBe(false));
    expect(everywhere).toBeDisabled();
    await user().click(within(options).getByRole('switch', { name: 'Detect language changes' }));
    await waitFor(() => expect(saved().readAloud.detectLanguages).toBe(true));
  });

  it('offers voices before they are downloaded and reads with one once it is here', async () => {
    const piper: TtsVoice = {
      key: 'piper-gb/alba',
      modelId: 'piper-gb',
      engine: 'piper',
      name: 'Alba',
      language: 'en-GB',
      gender: 'female',
    };
    const voices = [voice('af_heart', 'Heart', 'en-US'), piper, voice('ef_dora', 'Dora', 'es')];
    const saved = withSettings(DEFAULT_SETTINGS, voices);
    commands.ttsVoices.mockResolvedValue({ runtimeId: 'sherpa-runtime', voices });
    const runtime: AiModel = {
      ...MODEL,
      id: 'sherpa-runtime',
      name: 'Voice engine',
      kind: 'runtime',
      sizeBytes: 52_428_800,
      installed: false,
    };
    const alba: AiModel = {
      ...MODEL,
      id: 'piper-gb',
      name: 'Piper: Alba',
      engine: 'piper',
      sizeBytes: 67_108_864,
      installed: false,
    };
    let models = [{ ...MODEL, installed: false }, alba, runtime];
    commands.aiModels.mockImplementation(async () => models);
    commands.ttsLanguage.mockResolvedValue({ chosen: null, book: 'en' });
    // Rust downloads the voice, then chooses it.
    let downloaded = () => {};
    const chooses = commands.ttsUseVoice.getMockImplementation();
    commands.ttsUseVoice.mockImplementation(
      (key: string | null) =>
        new Promise((resolve) => {
          downloaded = () => resolve(chooses?.(key));
        }),
    );
    const reader = await openReader();
    await user().click(within(reader).getByRole('button', { name: 'Text settings' }));
    const panel = within(reader).getByRole('dialog', { name: 'Text settings' });
    await user().click(within(panel).getByRole('tab', { name: 'Read aloud' }));
    const options = within(panel).getByRole('list', { name: 'Reading aloud' });

    // The book's language's voices, each with what it takes (the voice engine comes with it).
    const voiceRow = await within(options).findByRole('button', { name: /^Voice/ });
    await user().click(voiceRow);
    const choices = await screen.findByRole('radiogroup', { name: 'Voice' });
    await waitFor(() =>
      expect(
        within(choices)
          .getAllByRole('radio')
          .map((radio) => radio.textContent),
      ).toEqual([
        "Device's voice",
        'HeartKokoro · American English · Not downloaded · 336 MB',
        'AlbaPiper · British English · Not downloaded · 114 MB',
      ]),
    );
    await user().click(within(choices).getByRole('radio', { name: /^Alba/ }));
    expect(commands.ttsUseVoice).toHaveBeenCalledWith('piper-gb/alba');

    // On its way: its progress in the options, with Cancel.
    emitEvent('aiModelEvent', {
      id: 'piper-gb',
      state: 'downloading',
      receivedBytes: 33_554_432,
      totalBytes: 67_108_864,
      error: null,
    });
    const arriving = await within(options).findByRole('listitem', {
      name: 'Downloading Piper: Alba',
    });
    expect(arriving).toHaveTextContent("50% · reads once it's here");
    expect(
      within(arriving).getByRole('button', { name: 'Cancel downloading Piper: Alba' }),
    ).toBeInTheDocument();

    // Here: Rust chooses it, and the reader shows it reading.
    models = [
      { ...MODEL, installed: false },
      { ...alba, installed: true },
      { ...runtime, installed: true },
    ];
    await act(async () => {
      downloaded();
      emitEvent('aiModelEvent', {
        id: 'piper-gb',
        state: 'installed',
        receivedBytes: 67_108_864,
        totalBytes: 67_108_864,
        error: null,
      });
      emitEvent('settingsChangedEvent', 'readAloud');
    });
    await waitFor(() => expect(saved().readAloud.engine).toBe('piper'));
    await waitFor(() => expect(voiceRow).toHaveTextContent('Alba'));
    expect(
      within(options).queryByRole('listitem', { name: 'Downloading Piper: Alba' }),
    ).not.toBeInTheDocument();
  });

  it('opens the same options from the reading-aloud bar, beside the AI button', async () => {
    withSettings(DEFAULT_SETTINGS);
    const reader = await openReader();
    await user().click(within(reader).getByRole('button', { name: 'Read aloud' }));
    const toolbar = within(reader).getByRole('toolbar', { name: 'Reading aloud' });
    // One row: the AI button keeps its place while the bar fits beside it.
    const row = within(reader).getByTestId('reader-bottom-row');
    expect(row).toContainElement(toolbar);
    expect(row).toContainElement(within(reader).getByTestId('reader-ai-button'));

    await user().click(within(toolbar).getByRole('button', { name: 'Reading-aloud options' }));
    const sheet = await screen.findByRole('dialog', { name: 'Reading aloud' });
    expect(within(sheet).getByRole('slider', { name: 'Volume' })).toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: /^Voice/ })).toHaveTextContent(
      "Device's voice",
    );
  });

  it('says when another voice reads a language none here speaks', async () => {
    withSettings(DEFAULT_SETTINGS);
    const reader = await openReader();
    await user().click(within(reader).getByRole('button', { name: 'Read aloud' }));
    emitEvent(
      'ttsEvent',
      update('speaking', 1, { notice: { kind: 'otherVoice', language: 'pl' }, language: 'en' }),
    );
    expect(await within(reader).findByRole('status')).toHaveTextContent(
      'No voice here reads Polish: another voice reads it.',
    );
  });
});
