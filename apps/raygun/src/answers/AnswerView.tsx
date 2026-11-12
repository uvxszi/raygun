import type { Answer, AnswerCredit } from '@raygun/bindings';
import { Copy, ExternalLink, History } from 'lucide-react';
import { Fragment, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { copyText } from '../music/mediaMenu';
import { Picture } from '../shell/Picture';
import { showToast } from '../shell/toastStore';
import { formatAnswerDate, openAnswerPage, useEngineName } from './answerText';
import { AnswerValue } from './AnswerValue';
import { useAppLanguage } from './useAnswers';

const Credit: FC<{ credit: AnswerCredit }> = ({ credit }) => (
  <span>
    {credit.url ? (
      <button
        type="button"
        onClick={() => openAnswerPage(credit.url as string)}
        className="hover:text-foreground underline underline-offset-2"
      >
        {credit.name}
      </button>
    ) : (
      credit.name
    )}
    {credit.license && (
      <>
        {' · '}
        {credit.licenseUrl ? (
          <button
            type="button"
            onClick={() => openAnswerPage(credit.licenseUrl as string)}
            className="hover:text-foreground underline underline-offset-2"
          >
            {credit.license}
          </button>
        ) : (
          credit.license
        )}
      </>
    )}
  </span>
);

/** Where an answer comes from: each source with its licence, and when it stands. */
export const AnswerCredits: FC<{ answer: Answer }> = ({ answer }) => {
  const { t } = useTranslation();
  const language = useAppLanguage();
  const asOf = answer.asOf ? (formatAnswerDate(answer.asOf, language) ?? answer.asOf.iso) : null;
  if (answer.credits.length === 0 && !asOf && !answer.kept) return null;
  return (
    <p className="text-muted flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
      {answer.credits.length > 0 && (
        <span>
          {t('answers.source')}:{' '}
          {answer.credits.map((credit, index) => (
            <Fragment key={`${credit.name}-${index}`}>
              {index > 0 && ', '}
              <Credit credit={credit} />
            </Fragment>
          ))}
        </span>
      )}
      {asOf && <span>{t('answers.asOf', { date: asOf })}</span>}
      {answer.kept && (
        <span className="flex items-center gap-1">
          <History aria-hidden size={12} />
          {t('answers.kept')}
        </span>
      )}
    </p>
  );
};

/** Copies an answer's result. */
export const CopyButton: FC<{ answer: Answer; className?: string }> = ({
  answer,
  className = '',
}) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-label={t('answers.copy')}
      title={t('answers.copy')}
      onClick={() => {
        copyText(answer.copy || answer.result.text);
        showToast(t('menu.copied'));
      }}
      className={`hover:bg-background text-muted hover:text-foreground rounded-full p-2 ${className}`}
    >
      <Copy aria-hidden size={18} />
    </button>
  );
};

type AnswerViewProps = {
  answer: Answer;
  /** Asks something else: a related question, a value that names something. */
  onAsk: (query: string) => void;
};

/**
 * One answer in full, as the Answers pill shows it: how the words were read, the result (with
 * its units toggle), its other forms and facts, the summary and picture, and where it's from.
 */
export const AnswerView: FC<AnswerViewProps> = ({ answer, onAsk }) => {
  const { t } = useTranslation();
  const engineName = useEngineName();
  const [unit, setUnit] = useState<string | null>(null);
  const toggled = answer.units.find((option) => option.symbol === unit);
  const result = toggled?.value ?? answer.result;
  const dateKind = answer.kind === 'date';
  return (
    <article
      data-testid="answer"
      aria-label={engineName(answer)}
      className="bg-surface flex flex-col gap-3 rounded-2xl p-4"
    >
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-muted text-xs font-semibold tracking-wide uppercase">
          {engineName(answer)}
        </h3>
        <CopyButton answer={answer} className="-m-2" />
      </div>
      {answer.interpretation && (
        <section aria-label={t('answers.interpretation')} className="flex flex-col gap-1">
          <h4 className="text-muted text-xs">{t('answers.interpretation')}</h4>
          <AnswerValue value={answer.interpretation} className="text-sm" />
        </section>
      )}
      <section aria-label={t('answers.result')} className="flex flex-col gap-1">
        <h4 className="text-muted text-xs">{t('answers.result')}</h4>
        <p className="text-2xl font-semibold break-words" data-testid="answer-result">
          {answer.approximate && !toggled && (
            <span aria-label={t('answers.approximately')} className="text-muted mr-1">
              ≈
            </span>
          )}
          <AnswerValue value={result} weekday={dateKind} onAsk={onAsk} />
        </p>
      </section>
      {answer.units.length > 1 && (
        <div role="group" aria-label={t('answers.units')} className="-mx-1 flex flex-wrap gap-1.5">
          {answer.units.map((option) => (
            <button
              key={option.symbol}
              type="button"
              aria-pressed={unit === option.symbol}
              onClick={() => setUnit(unit === option.symbol ? null : option.symbol)}
              className={`rounded-full px-3 py-1 text-sm ${
                unit === option.symbol
                  ? 'bg-primary/20 text-primary font-semibold'
                  : 'bg-background'
              }`}
            >
              {option.symbol}
            </button>
          ))}
        </div>
      )}
      {answer.summary && (
        <div className="flex gap-3">
          {answer.imageUrl && (
            <Picture
              src={answer.imageUrl}
              className="bg-background/40 size-24 shrink-0 rounded-xl"
            />
          )}
          <p className="selectable text-sm leading-relaxed whitespace-pre-line">{answer.summary}</p>
        </div>
      )}
      {answer.rows.length > 0 && (
        <dl className="grid grid-cols-[minmax(6rem,auto)_1fr] gap-x-4 gap-y-2 text-sm">
          {answer.rows.map((row, index) => (
            <Fragment key={`${row.key}-${row.label}-${index}`}>
              <dt className="text-muted">
                {row.key ? t(`answers.row.${row.key}`, { defaultValue: row.label }) : row.label}
              </dt>
              <dd className="min-w-0 break-words">
                <AnswerValue value={row.value} onAsk={onAsk} />
              </dd>
            </Fragment>
          ))}
        </dl>
      )}
      {answer.units.length > 1 && (
        <details className="text-sm">
          <summary className="text-muted cursor-pointer text-xs">
            {t('answers.conversions')}
          </summary>
          <ul className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 sm:grid-cols-3">
            {answer.units.map((option) => (
              <li key={option.symbol}>
                <AnswerValue value={option.value} />
              </li>
            ))}
          </ul>
        </details>
      )}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <AnswerCredits answer={answer} />
        {answer.url && (
          <button
            type="button"
            onClick={() => openAnswerPage(answer.url as string)}
            className="text-primary flex items-center gap-1 text-sm"
          >
            {t('answers.readMore')}
            <ExternalLink aria-hidden size={14} />
          </button>
        )}
      </div>
    </article>
  );
};
