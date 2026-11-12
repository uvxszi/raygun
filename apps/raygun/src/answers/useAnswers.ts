import { commands } from '@raygun/bindings';
import type { Answers } from '@raygun/bindings';
import { queryOptions, useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';

/** This device's time zone, for dates and times ("time in Tokyo" says the difference). */
const timeZone = () => {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || null;
  } catch {
    return null;
  }
};

/**
 * The answers to a query: for Search → All's card (`card`: only when the words read as a
 * question or a calculation) or the Answers pill (every engine). Rust reads the words at once,
 * so asking for every search costs nothing when it isn't a question.
 */
export const answersOptions = (query: string, card: boolean, language: string) =>
  queryOptions({
    queryKey: ['answers', card ? 'card' : 'full', language, query.trim()],
    queryFn: (): Promise<Answers> =>
      unwrapResult(
        commands.answersAsk({
          query: query.trim(),
          card,
          language,
          timeZone: timeZone(),
          utcOffsetMinutes: -new Date().getTimezoneOffset(),
          refresh: false,
        }),
      ),
    // Times and exchange rates move on; what's computed is cheap to ask again.
    staleTime: 60_000,
    retry: false,
  });

/** The app's language as the UI resolved it. */
export const useAppLanguage = () => {
  const { i18n } = useTranslation();
  return i18n.resolvedLanguage ?? i18n.language ?? 'en';
};

export const useAnswers = (query: string, card: boolean) => {
  const language = useAppLanguage();
  const settings = useSettings().data;
  // All's card waits for Settings, and shows only while it's on.
  const inAll = settings ? (settings.answers?.inAll ?? true) : false;
  return useQuery({
    ...answersOptions(query, card, language),
    enabled: query.trim() !== '' && (!card || inAll),
  });
};
