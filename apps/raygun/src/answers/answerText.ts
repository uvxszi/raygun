import { commands } from '@raygun/bindings';
import type { Answer, AnswerDate } from '@raygun/bindings';
import { useTranslation } from 'react-i18next';

/** A date (or a time of day) Rust gave, in the app's language; `null` when it can't be shown. */
export const formatAnswerDate = (
  date: AnswerDate,
  language: string,
  weekday = false,
): string | null => {
  const match = /^(\d{4,6})(?:-(\d{2}))?(?:-(\d{2}))?(?:T(\d{2}):(\d{2}))?$/.exec(date.iso);
  if (!match) return null;
  const [, year = '0', month = '1', day = '1', hours = '0', minutes = '0'] = match;
  const at = new Date(
    Date.UTC(Number(year), Number(month) - 1, Number(day), Number(hours), Number(minutes)),
  );
  if (Number.isNaN(at.getTime())) return null;
  const options: Intl.DateTimeFormatOptions = { timeZone: 'UTC' };
  switch (date.precision) {
    case 'minute':
      options.hour = 'numeric';
      options.minute = '2-digit';
      break;
    case 'day':
      options.year = 'numeric';
      options.month = 'long';
      options.day = 'numeric';
      if (weekday) options.weekday = 'long';
      break;
    case 'month':
      options.year = 'numeric';
      options.month = 'long';
      break;
    case 'year':
      options.year = 'numeric';
      break;
    default:
      return null;
  }
  try {
    return new Intl.DateTimeFormat(language, options).format(at);
  } catch {
    return null;
  }
};

/** Opens a page in Raygun's browser. */
export const openAnswerPage = (url: string) => void commands.answersOpenPage(url);

/** Who answered, as the app names its own engines. */
export const useEngineName = () => {
  const { t } = useTranslation();
  return (answer: Answer) => {
    switch (answer.engine) {
      case 'raygun':
        return t(`answers.engine.${answer.kind}`);
      case 'wikidata':
        return 'Wikidata';
      case 'wikipedia':
        return 'Wikipedia';
      case 'dictionary':
        return t('answers.engine.definition');
      case 'translation':
        return answer.engineName
          ? `${t('answers.engine.translation')} · ${answer.engineName}`
          : t('answers.engine.translation');
      default:
        return answer.engineName || t('answers.engine.other');
    }
  };
};
