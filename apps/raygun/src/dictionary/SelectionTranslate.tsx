import { commands } from '@raygun/bindings';
import type { TextTranslation, TranslateChoice } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { ArrowLeftRight, Languages, LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../ai/useAi';
import { LANGUAGES, nativeName } from '../i18n/languages';
import { unwrapResult } from '../lib/commandResult';
import { DeviceTranslatorNeeded } from '../readerAi/ReaderAi';
import { useReaderAiLayers } from '../readerAi/useReaderAi';
import { SourceIssue } from '../shell/SourceIssue';
import { PoweredByGoogle, TranslatorSelect } from '../translate/TranslatorSelect';
import { isGoogle, providerOf, useTranslatingProviders } from '../translate/translators';
import { ActionButton } from './ActionButton';

const SELECT = 'bg-surface min-w-0 flex-1 rounded-lg px-2 py-1.5 text-xs outline-none';

/** The remembered choice's query: a book's, else the last anywhere. */
const choiceKey = (titleId: string) => ['textTranslate', 'choice', titleId];

/** A tag's base language, for telling whether two tags are the same language. */
const baseOf = (tag: string) => (tag.split(/[-_]/)[0] ?? '').toLowerCase();

/** The app's languages, with tags it doesn't list (one detected, one remembered) added. */
const languagesWith = (...extra: string[]) => {
  const list = [...LANGUAGES];
  for (const tag of extra) {
    if (tag && !list.some((known) => known.toLowerCase() === tag.toLowerCase())) list.push(tag);
  }
  return list;
};

/** Who translated, as the line under the translation names them. */
const useTranslatorLine = (result: TextTranslation | undefined) => {
  const { t } = useTranslation();
  const models = useAiModels().data ?? [];
  if (!result) return '';
  if (result.kind) return t('translate.by', { name: result.translatorName });
  const names = result.models.map((id) => models.find((model) => model.id === id)?.name ?? id);
  return names.length > 0
    ? t('translate.byDeviceWith', { models: names.join(' → ') })
    : t('translate.byDevice');
};

/**
 * Translate in the word panel: closed, one button; open, the selection's translation with the
 * languages it's from (Auto shows what was detected) and into, a swap between them, and who
 * translates (this device, one of its models, or a provider from Settings → AI). The languages
 * and translator are remembered for the book, and as the last choice for others; changing any of
 * them translates again. Google's translations carry its attribution.
 */
export const SelectionTranslate: FC<{
  text: string;
  /** What the words are likely in: the dictionary's answer, the book's language. */
  hint: string;
  titleId: string;
  /** The app's language as shown, a BCP 47 tag. */
  appLanguage: string;
}> = ({ text, hint, titleId, appLanguage }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  const providers = useTranslatingProviders();
  const layer = useReaderAiLayers('novel').layers.find((one) => one.kind === 'translate' && one.to);
  const remembered = useQuery({
    queryKey: choiceKey(titleId),
    queryFn: () => unwrapResult(commands.textTranslateChoice(titleId || null)),
    staleTime: 60_000,
  });
  const [picked, setPicked] = useState<TranslateChoice | null>(null);
  const [open, setOpen] = useState(false);
  // Nothing picked yet: the book's translation layer, else the app's language.
  const choice: TranslateChoice = picked ??
    remembered.data ?? { from: '', to: layer?.to || appLanguage, translator: layer?.model ?? '' };
  const remember = useMutation({
    mutationFn: (next: TranslateChoice) =>
      unwrapResult(commands.textTranslateRemember(titleId || null, next)),
    onSuccess: () => void client.invalidateQueries({ queryKey: ['textTranslate', 'choice'] }),
  });
  const change = (next: Partial<TranslateChoice>) => {
    const updated = { ...choice, ...next };
    setPicked(updated);
    remember.mutate(updated);
  };
  const source = choice.from || hint;
  const same = source !== '' && baseOf(source) === baseOf(choice.to);
  const translation = useQuery({
    queryKey: ['textTranslate', text, choice.from, choice.to, choice.translator, hint],
    queryFn: () =>
      unwrapResult(
        commands.textTranslate({
          text,
          from: choice.from,
          to: choice.to,
          translator: choice.translator,
          hint,
        }),
      ),
    enabled: open && choice.to !== '' && !same,
    retry: false,
    staleTime: 10 * 60_000,
  });
  const result = translation.isFetching ? undefined : translation.data;
  const translatorLine = useTranslatorLine(result);
  const google = isGoogle(choice.translator, providers);

  if (!open) {
    return (
      <ActionButton
        icon={Languages}
        disabled={remembered.isPending}
        onClick={() => {
          setOpen(true);
          remember.mutate(choice);
        }}
      >
        {google ? t('translate.withGoogle') : t('dictionary.translate')}
      </ActionButton>
    );
  }

  const detected = choice.from === '' ? translation.data?.from || hint : '';
  const swapFrom = choice.from || detected;
  const provider = providerOf(choice.translator, providers);
  return (
    <section
      aria-label={t('translate.title')}
      data-testid="selection-translation"
      className="bg-background/60 flex basis-full flex-col gap-2 rounded-xl px-3 py-2 text-sm"
    >
      <div className="flex items-center gap-1">
        <select
          value={choice.from}
          aria-label={t('translate.from')}
          onChange={(event) => change({ from: event.target.value })}
          className={SELECT}
        >
          <option value="">
            {detected
              ? t('translate.autoDetected', { language: nativeName(detected) })
              : t('translate.auto')}
          </option>
          {languagesWith(choice.from).map((tag) => (
            <option key={tag} value={tag}>
              {nativeName(tag)}
            </option>
          ))}
        </select>
        <button
          type="button"
          aria-label={t('translate.swap')}
          title={t('translate.swap')}
          disabled={!swapFrom}
          onClick={() => change({ from: choice.to, to: swapFrom })}
          className="hover:bg-surface grid size-8 shrink-0 place-items-center rounded-full disabled:opacity-40"
        >
          <ArrowLeftRight aria-hidden size={15} />
        </button>
        <select
          value={choice.to}
          aria-label={t('translate.into')}
          onChange={(event) => change({ to: event.target.value })}
          className={SELECT}
        >
          {languagesWith(choice.to).map((tag) => (
            <option key={tag} value={tag}>
              {nativeName(tag)}
            </option>
          ))}
        </select>
      </div>
      {same && (
        <p className="text-muted text-xs">
          {t('translate.sameLanguage', { language: nativeName(choice.to) })}
        </p>
      )}
      {translation.isFetching && (
        <LoaderCircle
          role="status"
          aria-label={t('dictionary.translating')}
          className="text-muted animate-spin"
          size={16}
        />
      )}
      {result && (
        <div className="flex flex-col gap-0.5">
          <p className="selectable" lang={result.to}>
            {result.text}
          </p>
          <span className="text-muted text-2xs">{translatorLine}</span>
          {google && <PoweredByGoogle />}
        </div>
      )}
      {translation.error && !translation.isFetching && (
        <div className="flex flex-col gap-1">
          <SourceIssue compact source={provider?.name ?? t('translate.device')}>
            {translation.error.message}
          </SourceIssue>
          <button
            type="button"
            onClick={() => void navigate({ to: '/settings/ai', hash: 'translation' })}
            className="text-primary self-start text-xs font-semibold"
          >
            {t('dictionary.setUpTranslation')}
          </button>
        </div>
      )}
      {!provider && choice.to && <DeviceTranslatorNeeded to={choice.to} />}
      <label className="flex items-center gap-2 text-xs">
        <span className="text-muted shrink-0">{t('translate.translator')}</span>
        <TranslatorSelect
          value={choice.translator}
          label={t('translate.translator')}
          onChange={(translator) => change({ translator })}
          className={SELECT}
        />
      </label>
    </section>
  );
};
