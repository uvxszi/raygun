import type { AnswerText } from '@raygun/bindings';
import type { FC } from 'react';

import { MathFormula } from '../novel/MathFormula';
import { formatAnswerDate } from './answerText';
import { useAppLanguage } from './useAnswers';

type AnswerValueProps = {
  value: AnswerText;
  /** Days with their weekday (a date's answer). */
  weekday?: boolean;
  /** Asks what the value names (a director: who they are). */
  onAsk?: (query: string) => void;
  className?: string;
};

/**
 * Text in an answer: its formula drawn from MathML when it has one, a date in the app's language,
 * else the text; a value that names something can be asked about.
 */
export const AnswerValue: FC<AnswerValueProps> = ({ value, weekday, onAsk, className = '' }) => {
  const language = useAppLanguage();
  const date = value.date ? formatAnswerDate(value.date, language, weekday) : null;
  const shown = value.math ? (
    <MathFormula mathml={value.math.mathml} text={value.math.text} speech={value.math.speech} />
  ) : (
    (date ?? value.text)
  );
  if (value.query && onAsk) {
    const query = value.query;
    return (
      <button
        type="button"
        onClick={() => onAsk(query)}
        className={`text-primary text-left underline-offset-2 hover:underline ${className}`}
      >
        {shown}
      </button>
    );
  }
  return <span className={`selectable ${className}`}>{shown}</span>;
};
