import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Picture } from '../shell/Picture';
import { useEngineName } from './answerText';
import { AnswerValue } from './AnswerValue';
import { AnswerCredits, CopyButton } from './AnswerView';
import { useAnswers } from './useAnswers';

type AnswerCardProps = {
  query: string;
  /** Opens the Answers pill. */
  onMore: () => void;
  /** Asks something else the answer names. */
  onAsk: (query: string) => void;
};

/**
 * Search → All's answer, above the results, when the words are a question, a calculation or a
 * conversion. It's asked beside the sources and shows once ready; anything else (`Blue`,
 * `1984`) shows nothing and holds nothing up.
 */
export const AnswerCard: FC<AnswerCardProps> = ({ query, onMore, onAsk }) => {
  const { t } = useTranslation();
  const engineName = useEngineName();
  const answers = useAnswers(query, true);
  const answer = answers.data?.question ? answers.data.answers[0] : undefined;
  if (!answer) return null;
  return (
    // It comes once the answer is ready, over the results: it fades in rather than popping in.
    <section
      data-testid="answer-card"
      aria-label={t('answers.card')}
      className="bg-surface motion-fade flex flex-col gap-2 rounded-2xl p-4"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-1">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {engineName(answer)}
          </span>
          {answer.interpretation && (
            <AnswerValue value={answer.interpretation} className="text-muted text-sm" />
          )}
        </div>
        <CopyButton answer={answer} className="-m-1 shrink-0" />
      </div>
      <div className="flex gap-3">
        {answer.imageUrl && (
          <Picture src={answer.imageUrl} className="bg-background/40 size-16 shrink-0 rounded-xl" />
        )}
        <div className="flex min-w-0 flex-col gap-1">
          <p className="text-xl font-semibold break-words" data-testid="answer-card-result">
            {answer.approximate && (
              <span aria-label={t('answers.approximately')} className="text-muted mr-1">
                ≈
              </span>
            )}
            <AnswerValue value={answer.result} weekday={answer.kind === 'date'} onAsk={onAsk} />
          </p>
          {answer.summary && <p className="text-muted line-clamp-3 text-sm">{answer.summary}</p>}
        </div>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <AnswerCredits answer={answer} />
        <button
          type="button"
          onClick={onMore}
          className="text-primary ml-auto flex items-center gap-0.5 text-sm"
        >
          {t('answers.more')}
          <ChevronRight aria-hidden size={16} />
        </button>
      </div>
    </section>
  );
};
