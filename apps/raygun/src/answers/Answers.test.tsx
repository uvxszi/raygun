import type { Answer, AnswerRequest, Answers } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { formatAnswerDate } from './answerText';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const answer = (overrides: Partial<Answer>): Answer => ({
  engine: 'raygun',
  engineName: '',
  kind: 'calculation',
  interpretation: null,
  result: { text: '', math: null, date: null, query: null },
  approximate: false,
  rows: [],
  units: [],
  summary: null,
  imageUrl: null,
  url: null,
  credits: [],
  asOf: null,
  copy: '',
  kept: false,
  ...overrides,
});

const answers = (query: string, found: Answer[], extra: Partial<Answers> = {}): Answers => ({
  query,
  question: found.length > 0,
  answers: found,
  related: [],
  offline: false,
  needsTranslator: false,
  issues: [],
  ...extra,
});

const MILES = answer({
  kind: 'conversion',
  interpretation: { text: '5 km → miles', math: null, date: null, query: null },
  result: { text: '3.1068559612 miles', math: null, date: null, query: null },
  approximate: true,
  copy: '3.1068559612 miles',
  units: [
    {
      symbol: 'm',
      system: 'metric',
      value: { text: '5,000 m', math: null, date: null, query: null },
    },
    {
      symbol: 'ft',
      system: 'imperial',
      value: { text: '16,404.2 ft', math: null, date: null, query: null },
    },
    {
      symbol: 'mi',
      system: 'imperial',
      value: { text: '3.10686 mi', math: null, date: null, query: null },
    },
  ],
});

const POPULATION = answer({
  engine: 'wikidata',
  kind: 'fact',
  interpretation: {
    text: 'Population of Japan (island country in East Asia)',
    math: null,
    date: null,
    query: null,
  },
  result: { text: '123,780,000', math: null, date: null, query: null },
  copy: '123,780,000',
  asOf: { iso: '2025-10-01', precision: 'day' },
  url: 'https://www.wikidata.org/wiki/Q17',
  credits: [
    {
      name: 'Wikidata',
      url: 'https://www.wikidata.org/wiki/Q17',
      license: 'CC0 1.0',
      licenseUrl: 'https://creativecommons.org/publicdomain/zero/1.0/',
    },
  ],
});

describe('Answers in Search', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('answers a calculation above All, and opens it in full on the Answers pill', async () => {
    const user = userEvent.setup();
    commands.answersAsk.mockImplementation(async (request: AnswerRequest) =>
      ok(answers(request.query, request.query === '5 km to miles' ? [MILES] : [])),
    );
    await AppShellWrapper.mount('/search?q=5%20km%20to%20miles');
    const card = await screen.findByTestId('answer-card');
    expect(within(card).getByTestId('answer-card-result')).toHaveTextContent('≈3.1068559612 miles');
    expect(within(card).getByText('5 km → miles')).toBeInTheDocument();
    expect(commands.answersAsk).toHaveBeenCalledWith(
      expect.objectContaining({ query: '5 km to miles', card: true, language: 'en' }),
    );

    // Copy takes the result.
    await user.click(within(card).getByRole('button', { name: 'Copy the result' }));
    expect(await screen.findByText('Copied')).toBeInTheDocument();

    await user.click(within(card).getByRole('button', { name: 'More answers' }));
    expect(screen.getByRole('tab', { name: 'Answers' })).toHaveAttribute('aria-selected', 'true');
    const full = await screen.findByTestId('answers-results');
    await waitFor(() =>
      expect(commands.answersAsk).toHaveBeenCalledWith(
        expect.objectContaining({ query: '5 km to miles', card: false }),
      ),
    );
    const view = await within(full).findByTestId('answer');
    expect(within(view).getByText('Input interpretation')).toBeInTheDocument();

    // The units toggle shows the result in another unit, and back.
    const units = within(view).getByRole('group', { name: 'Units' });
    await user.click(within(units).getByRole('button', { name: 'ft' }));
    expect(within(view).getByTestId('answer-result')).toHaveTextContent('16,404.2 ft');
    await user.click(within(units).getByRole('button', { name: 'ft' }));
    expect(within(view).getByTestId('answer-result')).toHaveTextContent('3.1068559612 miles');
  });

  it('shows nothing above a media search', async () => {
    await AppShellWrapper.mount('/search?q=1984');
    await screen.findByTestId('all-results');
    await waitFor(() =>
      expect(commands.answersAsk).toHaveBeenCalledWith(
        expect.objectContaining({ query: '1984', card: true }),
      ),
    );
    expect(screen.queryByTestId('answer-card')).not.toBeInTheDocument();
  });

  it('credits facts with their licence, and asks related questions', async () => {
    const user = userEvent.setup();
    commands.answersAsk.mockImplementation(async (request: AnswerRequest) =>
      ok(
        request.query === 'population of japan'
          ? answers(request.query, [POPULATION], {
              related: [{ label: 'Capital of Japan', query: 'capital of Japan' }],
            })
          : answers(request.query, [], { question: true }),
      ),
    );
    await AppShellWrapper.mount('/search?type=answers&q=population%20of%20japan');
    const view = await screen.findByTestId('answer');
    expect(within(view).getByTestId('answer-result')).toHaveTextContent('123,780,000');
    expect(within(view).getByText('As of October 1, 2025')).toBeInTheDocument();

    await user.click(within(view).getByRole('button', { name: 'CC0 1.0' }));
    expect(commands.answersOpenPage).toHaveBeenCalledWith(
      'https://creativecommons.org/publicdomain/zero/1.0/',
    );
    await user.click(within(view).getByRole('button', { name: 'Read more' }));
    expect(commands.answersOpenPage).toHaveBeenLastCalledWith('https://www.wikidata.org/wiki/Q17');

    const related = screen.getByRole('region', { name: 'Related' });
    await user.click(within(related).getByRole('button', { name: 'Capital of Japan' }));
    await waitFor(() =>
      expect(commands.answersAsk).toHaveBeenLastCalledWith(
        expect.objectContaining({ query: 'capital of Japan', card: false }),
      ),
    );
    expect(await screen.findByText('No answer for “capital of Japan”.')).toBeInTheDocument();
  });

  it('suggests questions before anything is typed, and points to translators', async () => {
    const user = userEvent.setup();
    commands.answersAsk.mockImplementation(async (request: AnswerRequest) =>
      ok(answers(request.query, [], { question: true, needsTranslator: true })),
    );
    await AppShellWrapper.mount('/search?type=answers');
    const results = await screen.findByTestId('answers-results');
    const examples = within(results).getByRole('group', { name: 'Examples' });
    await user.click(within(examples).getByRole('button', { name: '15% of 80' }));
    expect(await screen.findByText(/Translating needs a translator/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Set up translation' })).toBeInTheDocument();
  });

  it('turns the card in All off in Settings → Browse', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/browse');
    const toggle = await screen.findByRole('switch', { name: /Answers in All/ });
    await user.click(toggle);
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'answers',
      value: { inAll: false, online: true },
    });
  });

  it('writes dates in the app language', () => {
    expect(formatAnswerDate({ iso: '2027-01-01', precision: 'day' }, 'en', true)).toBe(
      'Friday, January 1, 2027',
    );
    expect(formatAnswerDate({ iso: '2026-11-11T23:30', precision: 'minute' }, 'en')).toBe(
      '11:30 PM',
    );
    expect(formatAnswerDate({ iso: '1879-03', precision: 'month' }, 'en')).toBe('March 1879');
    expect(formatAnswerDate({ iso: '1870', precision: 'decade' }, 'en')).toBeNull();
  });
});
