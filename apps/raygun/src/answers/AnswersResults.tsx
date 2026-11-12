import { Link } from '@tanstack/react-router';
import { Languages } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { OfflineNote } from '../network/OfflineNote';
import { NeedsSetupNote } from '../shell/NeedsSetupNote';
import { SourceIssue } from '../shell/SourceIssue';
import { AnswerView } from './AnswerView';
import { useAnswers } from './useAnswers';

/** What the Answers pill offers to try before anything is typed. */
const EXAMPLES = [
  '15% of 80',
  '5 km in miles',
  '100 USD to EUR',
  'time in Tokyo',
  'days until 1 January',
  'solve x^2 - 5x + 6 = 0',
  'population of Japan',
  'who directed Spirited Away',
  'define serendipity',
];

type AnswersResultsProps = {
  query: string;
  /** Searches the Answers pill for other words. */
  onAsk: (query: string) => void;
};

/**
 * Search → Answers: every engine's answer to the words, in full: calculations, conversions,
 * dates and times worked out on this device, facts from Wikidata, summaries from Wikipedia,
 * definitions, translations and extensions' answers, each with its sources.
 */
export const AnswersResults: FC<AnswersResultsProps> = ({ query, onAsk }) => {
  const { t } = useTranslation();
  const blank = query.trim() === '';
  const answers = useAnswers(query, false);
  if (blank) {
    return (
      <div data-testid="answers-results" className="flex flex-col gap-3">
        <p className="text-muted text-sm">{t('answers.hint')}</p>
        <div role="group" aria-label={t('answers.examples')} className="flex flex-wrap gap-2">
          {EXAMPLES.map((example) => (
            <button
              key={example}
              type="button"
              onClick={() => onAsk(example)}
              className="bg-surface rounded-full px-3 py-1.5 text-sm"
            >
              {example}
            </button>
          ))}
        </div>
        <NeedsSetupNote kinds={['answers']} />
      </div>
    );
  }
  const data = answers.data;
  return (
    <div data-testid="answers-results" className="flex flex-col gap-4">
      {!data && !answers.error && <p className="text-muted text-sm">{t('answers.working')}</p>}
      {answers.error && (
        <SourceIssue onRetry={() => void answers.refetch()}>{answers.error.message}</SourceIssue>
      )}
      {data?.answers.map((answer, index) => (
        <AnswerView key={`${answer.engine}-${index}`} answer={answer} onAsk={onAsk} />
      ))}
      {data?.needsTranslator && (
        <div className="bg-surface/70 flex flex-wrap items-center gap-2 rounded-xl px-4 py-3 text-sm">
          <Languages aria-hidden size={16} className="text-muted shrink-0" />
          <span className="min-w-0 flex-1">{t('answers.needsTranslator')}</span>
          <Link to="/settings/ai" hash="translation" className="text-primary font-semibold">
            {t('answers.setUpTranslator')}
          </Link>
        </div>
      )}
      {data?.offline && <OfflineNote compact />}
      {data && data.answers.length === 0 && !data.needsTranslator && !data.offline && (
        <p className="text-muted text-sm">{t('answers.nothing', { query: query.trim() })}</p>
      )}
      {data?.issues.map((issue) => (
        <SourceIssue key={issue} compact>
          {issue}
        </SourceIssue>
      ))}
      {data && data.related.length > 0 && (
        <section aria-label={t('answers.related')} className="flex flex-col gap-2">
          <h2 className="font-semibold">{t('answers.related')}</h2>
          <div className="flex flex-wrap gap-2">
            {data.related.map((suggestion) => (
              <button
                key={suggestion.query}
                type="button"
                onClick={() => onAsk(suggestion.query)}
                className="bg-surface rounded-full px-3 py-1.5 text-sm"
              >
                {suggestion.label}
              </button>
            ))}
          </div>
        </section>
      )}
      <NeedsSetupNote kinds={['answers']} />
    </div>
  );
};
