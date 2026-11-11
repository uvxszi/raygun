import type { DictionaryLookup, NovelChapter } from '@raygun/bindings';
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const chapterInfo = (id: string, name: string) => ({
  id,
  name,
  number: null,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
});

const CHAPTER: NovelChapter = {
  titleId: 't1',
  title: 'Frankenstein',
  chapter: chapterInfo('c1', 'Letter 1'),
  blocks: [
    { kind: 'heading', text: 'Letter 1' },
    { kind: 'paragraph', text: 'It was a dreary night of November.' },
    { kind: 'paragraph', text: 'Rain fell on the panes.' },
  ],
  previousChapterId: null,
  nextChapterId: 'c2',
};

const DREARY: DictionaryLookup = {
  query: 'dreary',
  word: 'dreary',
  language: 'en',
  languageName: 'English',
  formOf: null,
  pronunciations: [
    { ipa: '/ˈdɹɪəɹi/', label: 'UK', audioUrl: null },
    { ipa: null, label: 'US', audioUrl: 'http://127.0.0.1:1/media/t/https/example.org/dreary.mp3' },
  ],
  meanings: [
    {
      partOfSpeech: 'adjective',
      definitions: [
        { text: 'Drab; dull; cheerless.', examples: ['a dreary afternoon'] },
        { text: 'Grievous; dire.', examples: [] },
      ],
      synonyms: ['bleak', 'gloomy'],
      antonyms: ['cheerful'],
      translations: [{ gloss: 'drab', language: 'de', terms: ['trostlos'] }],
    },
  ],
  etymology: 'From Old English drēoriġ.',
  credits: [
    {
      source: 'wiktionary',
      name: 'Wiktionary',
      url: 'https://en.wiktionary.org/wiki/dreary#English',
      license: { name: 'CC BY-SA 4.0', url: 'https://creativecommons.org/licenses/by-sa/4.0/' },
    },
  ],
  onlineUrl: 'https://en.wiktionary.org/w/index.php?search=dreary',
  kept: false,
  offline: false,
  issues: [],
};

/** Selects `word` in the paragraph that holds it, as a long press or a mouse would. */
const select = (paragraph: HTMLElement, word: string) => {
  const text = paragraph.firstChild as Text;
  const start = (text.textContent ?? '').indexOf(word);
  const range = document.createRange();
  range.setStart(text, start);
  range.setEnd(text, start + word.length);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
  act(() => {
    document.dispatchEvent(new Event('selectionchange'));
  });
};

describe('Word lookups in the book reader', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    Element.prototype.scrollIntoView = vi.fn();
    commands.novelChapter.mockResolvedValue(ok(CHAPTER));
  });

  afterEach(() => {
    window.getSelection()?.removeAllRanges();
  });

  it('looks up a selected word and shows what the dictionaries say', async () => {
    const user = userEvent.setup();
    commands.dictionaryLookup.mockImplementation(async (request: { text: string }) =>
      ok(
        request.text === 'dreary'
          ? DREARY
          : {
              ...DREARY,
              query: request.text,
              word: '',
              pronunciations: [],
              meanings: [],
              etymology: null,
              credits: [],
            },
      ),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    const paragraph = await within(reader).findByText('It was a dreary night of November.');
    expect(screen.getByTestId('novel-scroll')).toHaveClass('selectable');

    select(paragraph, 'dreary');
    const lookup = await screen.findByTestId('word-lookup', {}, { timeout: 2000 });
    expect(commands.dictionaryLookup).toHaveBeenCalledWith({
      text: 'dreary',
      titleId: 't1',
      appLanguage: 'en',
      refresh: false,
    });
    const panel = within(lookup);
    expect(await panel.findByText('Drab; dull; cheerless.')).toBeInTheDocument();
    expect(panel.getByRole('heading', { name: 'dreary' })).toBeInTheDocument();
    expect(panel.getByText('/ˈdɹɪəɹi/')).toBeInTheDocument();
    expect(panel.getByRole('button', { name: 'Play the pronunciation (US)' })).toBeInTheDocument();
    expect(panel.getByText('“a dreary afternoon”')).toBeInTheDocument();
    expect(panel.getByRole('button', { name: 'bleak' })).toBeInTheDocument();
    expect(panel.getByText('trostlos')).toBeInTheDocument();
    expect(panel.getByText('From Old English drēoriġ.')).toBeInTheDocument();

    // The credit links to the entry, the licence to its text.
    await user.click(panel.getByRole('button', { name: 'Wiktionary' }));
    expect(commands.dictionaryOpenPage).toHaveBeenCalledWith(
      'https://en.wiktionary.org/wiki/dreary#English',
    );
    await user.click(panel.getByRole('button', { name: 'CC BY-SA 4.0' }));
    expect(commands.dictionaryOpenPage).toHaveBeenLastCalledWith(
      'https://creativecommons.org/licenses/by-sa/4.0/',
    );

    // A synonym is looked up in place, and Back returns to the word before.
    await user.click(panel.getByRole('button', { name: 'bleak' }));
    expect(commands.dictionaryLookup).toHaveBeenLastCalledWith(
      expect.objectContaining({ text: 'bleak' }),
    );
    expect(await panel.findByText('No dictionary knows “bleak”.')).toBeInTheDocument();
    await user.click(panel.getByRole('button', { name: 'Back to the word before' }));
    expect(await panel.findByText('Drab; dull; cheerless.')).toBeInTheDocument();

    await user.click(panel.getByRole('button', { name: 'Close' }));
    expect(screen.queryByTestId('word-lookup')).not.toBeInTheDocument();
    // Closing it doesn't toggle the reader's bars.
    expect(within(reader).getByRole('button', { name: 'Text settings' })).toBeInTheDocument();
  });

  it('reads aloud from the selection, and searches the book for it', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.dictionaryLookup.mockResolvedValue(ok(DREARY));
    commands.novelSearchBook.mockResolvedValue(
      ok({
        hits: [
          {
            chapterId: 'c1',
            chapterName: 'Letter 1',
            block: 1,
            before: 'It was a ',
            matched: 'dreary',
            after: ' night',
          },
          {
            chapterId: 'c4',
            chapterName: 'Letter 4',
            block: 7,
            before: 'the ',
            matched: 'dreary',
            after: ' sea',
          },
        ],
        searched: 2,
        chapters: 24,
        more: false,
      }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    const paragraph = await within(reader).findByText('It was a dreary night of November.');
    await within(reader).findByRole('button', { name: 'Read aloud' });

    select(paragraph, 'dreary');
    let lookup = await screen.findByTestId('word-lookup', {}, { timeout: 2000 });
    await user.click(within(lookup).getByRole('button', { name: 'Read aloud from here' }));
    expect(commands.ttsSpeak).toHaveBeenCalledWith(
      expect.objectContaining({
        paragraphs: expect.arrayContaining([
          { id: 1, text: 'It was a dreary night of November.' },
          { id: 2, text: 'Rain fell on the panes.' },
        ]),
        from: 1,
        resume: false,
        rate: 1,
      }),
    );
    expect(screen.queryByTestId('word-lookup')).not.toBeInTheDocument();

    window.getSelection()?.removeAllRanges();
    act(() => {
      document.dispatchEvent(new Event('selectionchange'));
    });
    select(paragraph, 'dreary');
    lookup = await screen.findByTestId('word-lookup', {}, { timeout: 2000 });
    await user.click(within(lookup).getByRole('button', { name: 'Search in book' }));
    const sheet = await screen.findByRole('dialog', { name: 'Search in book' });
    expect(commands.novelSearchBook).toHaveBeenCalledWith('t1', 'c1', 'dreary');
    expect(
      await within(sheet).findByText(
        'Searched 2 of 24 chapters: the ones on this device (downloaded, or open).',
      ),
    ).toBeInTheDocument();
    // A place in another chapter opens it at the paragraph.
    await user.click(within(sheet).getByText('Letter 4'));
    expect(commands.novelChapter).toHaveBeenLastCalledWith('t1', 'c4');
  });

  it('keeps the text unselectable while lookups are off', async () => {
    commands.settingsGet.mockResolvedValue(
      ok({
        ...DEFAULT_SETTINGS,
        dictionary: { enabled: false, order: [], off: [], language: '' },
      }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    const paragraph = await within(reader).findByText('It was a dreary night of November.');
    expect(screen.getByTestId('novel-scroll')).toHaveClass('select-none');
    select(paragraph, 'dreary');
    await act(() => new Promise((resolve) => setTimeout(resolve, 500)));
    expect(screen.queryByTestId('word-lookup')).not.toBeInTheDocument();
    expect(commands.dictionaryLookup).not.toHaveBeenCalled();
  });

  it('sets the dictionaries and their order in Settings → Reader', async () => {
    const user = userEvent.setup();
    commands.dictionarySources.mockResolvedValue(
      ok([
        { id: 'wiktionary', name: 'Wiktionary', languages: [], builtIn: true, enabled: true },
        {
          id: 'freeDictionary',
          name: 'Free Dictionary API',
          languages: ['en'],
          builtIn: true,
          enabled: true,
        },
      ]),
    );
    await AppShellWrapper.mount('/settings/reader');
    const group = await screen.findByRole('region', { name: 'Dictionary' });
    await within(group).findByText('Free Dictionary API');
    await user.click(within(group).getByRole('button', { name: 'Move Free Dictionary API up' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'dictionary',
      value: { enabled: true, order: ['freeDictionary', 'wiktionary'], off: [], language: '' },
    });
    await user.click(within(group).getByRole('checkbox', { name: /Wiktionary/ }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith(
      expect.objectContaining({
        key: 'dictionary',
        value: expect.objectContaining({ off: ['wiktionary'] }),
      }),
    );
    await user.click(within(group).getByRole('switch', { name: 'Look up words' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith(
      expect.objectContaining({ value: expect.objectContaining({ enabled: false }) }),
    );
  });
});
