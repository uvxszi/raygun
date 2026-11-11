import { commands } from '@raygun/bindings';
import type { DictionaryLookup, DictionaryMeaning } from '@raygun/bindings';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Copy,
  ExternalLink,
  Languages,
  LoaderCircle,
  RotateCcw,
  Search,
  Volume2,
  X,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import type { CSSProperties, FC, ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { nativeName } from '../i18n/languages';
import { unwrapResult } from '../lib/commandResult';
import { copyText } from '../music/mediaMenu';
import { OfflineNote } from '../network/OfflineNote';
import { useReaderAiLayers } from '../readerAi/useReaderAi';
import { useBackLayer } from '../shell/backLayers';
import { safeInsets } from '../shell/safeArea';
import { showToast } from '../shell/toastStore';
import { useWideScreen } from '../shell/useWideScreen';
import { playPronunciation, stopPronunciation } from './pronunciation';
import { isLookupText } from './useWordSelection';
import type { SelectionBox, WordSelection } from './useWordSelection';

/** Senses shown per part of speech before Show all. */
const FIRST_SENSES = 3;
/** The floating panel's width on wide windows, in px at Interface size 100 %. */
const PANEL_WIDTH = 380;
const MARGIN = 8;

type Size = { width: number; height: number };

/** The panel's size as drawn, followed as the answer fills it. */
const usePanelSize = (panel: HTMLElement | null): Size => {
  const [size, setSize] = useState<Size>({ width: PANEL_WIDTH, height: 0 });
  useEffect(() => {
    if (!panel || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() =>
      setSize({ width: panel.offsetWidth, height: panel.offsetHeight }),
    );
    observer.observe(panel);
    return () => observer.disconnect();
  }, [panel]);
  return size;
};

/**
 * Where the panel goes: beside the selection on wide windows (under it when it fits, else
 * above), else along the bottom of the screen, or the top when the selection is in the lower half.
 */
const placement = (box: SelectionBox, wide: boolean, size: Size): CSSProperties => {
  const safe = safeInsets();
  if (!wide) {
    // Across the page area, as the reader's own bars are.
    const sides = {
      left: `calc(var(--main-left, 0px) + ${MARGIN}px)`,
      right: `calc(var(--main-right, 0px) + ${MARGIN}px)`,
    };
    return box.top > window.innerHeight / 2
      ? { top: `calc(${safe.top + MARGIN}px + var(--app-bar, 0px))`, ...sides }
      : { bottom: safe.bottom + MARGIN, ...sides };
  }
  const below = window.innerHeight - safe.bottom - box.bottom - MARGIN * 2;
  const above = box.top - safe.top - MARGIN * 2;
  const under = size.height <= below || below >= above;
  const top = under
    ? box.bottom + MARGIN
    : Math.max(safe.top + MARGIN, box.top - MARGIN - size.height);
  const left = Math.min(
    Math.max(safe.left + MARGIN, box.left),
    window.innerWidth - safe.right - MARGIN - size.width,
  );
  return { top, left, maxHeight: Math.max(under ? below : above, 160) };
};

/** Plays one recording. */
const PlayButton: FC<{ url: string; label: string | null }> = ({ url, label }) => {
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);
  return (
    <button
      type="button"
      onClick={() => {
        setFailed(false);
        void playPronunciation(url).catch(() => setFailed(true));
      }}
      aria-label={label ? t('dictionary.playLabelled', { label }) : t('dictionary.play')}
      className={`hover:bg-background inline-flex items-center gap-1 rounded-full px-2 py-1 text-xs ${failed ? 'text-muted line-through' : ''}`}
    >
      <Volume2 aria-hidden size={15} />
      {label && <span>{label}</span>}
    </button>
  );
};

/** Words in a row (synonyms, antonyms), each looked up when tapped. */
const WordRow: FC<{ title: string; words: string[]; onLookUp: (word: string) => void }> = ({
  title,
  words,
  onLookUp,
}) => (
  <p className="text-sm">
    <span className="text-muted">{title}: </span>
    {words.map((word, index) => (
      <span key={word}>
        {index > 0 && ', '}
        <button
          type="button"
          onClick={() => onLookUp(word)}
          className="text-primary hover:underline"
        >
          {word}
        </button>
      </span>
    ))}
  </p>
);

/** The part of speech as shown: the app's word for the common ones, else the source's. */
const usePartName = () => {
  const { t } = useTranslation();
  return (part: string) => {
    const key = part.replace(/\s+(\w)/g, (_, letter: string) => letter.toUpperCase());
    return t(`dictionary.parts.${key}`, { defaultValue: part });
  };
};

const Meaning: FC<{
  meaning: DictionaryMeaning;
  all: boolean;
  onLookUp: (word: string) => void;
}> = ({ meaning, all, onLookUp }) => {
  const { t } = useTranslation();
  const partName = usePartName();
  const senses = all ? meaning.definitions : meaning.definitions.slice(0, FIRST_SENSES);
  return (
    <section className="flex flex-col gap-1.5" aria-label={partName(meaning.partOfSpeech)}>
      {meaning.partOfSpeech && (
        <h3 className="text-muted text-xs font-semibold tracking-wide uppercase">
          {partName(meaning.partOfSpeech)}
        </h3>
      )}
      <ol className="flex list-decimal flex-col gap-1.5 pl-5 text-sm">
        {senses.map((sense, index) => (
          <li key={index}>
            <span>{sense.text}</span>
            {sense.examples[0] && (
              <span className="text-muted mt-0.5 block italic">“{sense.examples[0]}”</span>
            )}
          </li>
        ))}
      </ol>
      {meaning.synonyms.length > 0 && (
        <WordRow
          title={t('dictionary.synonyms')}
          words={meaning.synonyms.slice(0, 8)}
          onLookUp={onLookUp}
        />
      )}
      {meaning.antonyms.length > 0 && (
        <WordRow
          title={t('dictionary.antonyms')}
          words={meaning.antonyms.slice(0, 8)}
          onLookUp={onLookUp}
        />
      )}
      {meaning.translations.slice(0, all ? 6 : 2).map((translation, index) => (
        <p key={index} className="text-sm">
          <span className="text-muted">
            {t('dictionary.translationsIn', { language: nativeName(translation.language) })}
            {translation.gloss ? ` (${translation.gloss})` : ''}:{' '}
          </span>
          <span className="selectable">{translation.terms.join(', ')}</span>
        </p>
      ))}
    </section>
  );
};

/** The words translated with the reader's translator (the book's AI layers', else this
 * device's choice), into the layer's language or the app's. */
const TranslateRow: FC<{ text: string; from: string; appLanguage: string }> = ({
  text,
  from,
  appLanguage,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const layer = useReaderAiLayers('novel').layers.find((layer) => layer.to);
  const to = layer?.to || appLanguage;
  const translate = useMutation({
    mutationFn: () =>
      unwrapResult(
        commands.readerAiTranslate({ texts: [text], from, to, model: layer?.model ?? '' }),
      ).then((texts) => texts[0] ?? ''),
  });
  if (to.split('-')[0] === from.split('-')[0] && !layer) return null;
  if (translate.isIdle) {
    return (
      <ActionButton icon={Languages} onClick={() => translate.mutate()}>
        {t('dictionary.translate')}
      </ActionButton>
    );
  }
  return (
    <div className="bg-background/60 basis-full rounded-xl px-3 py-2 text-sm" role="status">
      <span className="text-muted block text-xs">
        {t('dictionary.translationInto', { language: nativeName(to) })}
      </span>
      {translate.isPending && (
        <LoaderCircle aria-label={t('dictionary.translating')} className="animate-spin" size={16} />
      )}
      {translate.data && <span className="selectable">{translate.data}</span>}
      {translate.error && (
        <span className="flex flex-col gap-1">
          <span className="text-muted">{translate.error.message}</span>
          <button
            type="button"
            onClick={() => void navigate({ to: '/settings/ai', hash: 'translation' })}
            className="text-primary self-start text-xs font-semibold"
          >
            {t('dictionary.setUpTranslation')}
          </button>
        </span>
      )}
    </div>
  );
};

const ActionButton: FC<{
  icon: typeof Copy;
  onClick: () => void;
  children: ReactNode;
}> = ({ icon: Icon, onClick, children }) => (
  <button
    type="button"
    onClick={onClick}
    className="bg-background/60 hover:bg-background inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-medium"
  >
    <Icon aria-hidden size={15} />
    {children}
  </button>
);

/** Who answered, with the licence their text comes under and a link to the full entry. */
const Credits: FC<{ lookup: DictionaryLookup }> = ({ lookup }) => {
  const { t } = useTranslation();
  if (lookup.credits.length === 0) return null;
  return (
    <p className="text-muted text-xs">
      {t('dictionary.from')}{' '}
      {lookup.credits.map((credit, index) => (
        <span key={credit.source}>
          {index > 0 && ' · '}
          {credit.url ? (
            <button
              type="button"
              onClick={() => void commands.dictionaryOpenPage(credit.url as string)}
              className="hover:text-foreground underline"
            >
              {credit.name}
            </button>
          ) : (
            credit.name
          )}
          {credit.license && (
            <>
              {' ('}
              {credit.license.url ? (
                <button
                  type="button"
                  onClick={() => void commands.dictionaryOpenPage(credit.license?.url as string)}
                  className="hover:text-foreground underline"
                >
                  {credit.license.name}
                </button>
              ) : (
                credit.license.name
              )}
              {')'}
            </>
          )}
        </span>
      ))}
    </p>
  );
};

export type WordLookupProps = {
  selection: WordSelection;
  titleId: string;
  /** The app's language as shown, a BCP 47 tag. */
  appLanguage: string;
  onClose: () => void;
  onSearchBook: (text: string) => void;
  /** Reads aloud from the selection's paragraph, when reading aloud is there. */
  onReadAloud?: () => void;
};

/**
 * What a selection in the book means: the dictionaries' answer (pronunciation, senses with
 * examples, synonyms and antonyms, translations, the word's history, who said it), and what to do
 * with the words (copy, search the book, translate, read aloud, look them up online). Beside the
 * selection on wide windows; along an edge of a phone's screen, clear of the selection.
 */
export const WordLookup: FC<WordLookupProps> = ({
  selection,
  titleId,
  appLanguage,
  onClose,
  onSearchBook,
  onReadAloud,
}) => {
  const { t } = useTranslation();
  const wide = useWideScreen();
  const [panel, setPanel] = useState<HTMLDivElement | null>(null);
  const size = usePanelSize(panel);
  const style = placement(selection.box, wide, size);
  // A synonym or a lemma tapped is looked up in place.
  const [asked, setAsked] = useState<string[]>([selection.text]);
  const query = asked[asked.length - 1] ?? selection.text;
  const [all, setAll] = useState(false);
  const lookup = isLookupText(query);
  const answer = useQuery({
    queryKey: ['dictionary', titleId, appLanguage, query],
    queryFn: () =>
      unwrapResult(
        commands.dictionaryLookup({ text: query, titleId, appLanguage, refresh: false }),
      ),
    enabled: lookup,
    staleTime: 5 * 60_000,
    retry: false,
  });
  useBackLayer(true, onClose);
  useEffect(() => () => stopPronunciation(), []);
  const lookUp = (word: string) => {
    setAll(false);
    setAsked((list) => [...list, word]);
  };
  const data = answer.data;
  const found = data && data.meanings.length > 0;
  const followed = data?.formOf && data.word !== data.formOf.form;
  const senses = data?.meanings.reduce((sum, meaning) => sum + meaning.definitions.length, 0) ?? 0;
  const ipa = data?.pronunciations.filter((sound) => sound.ipa) ?? [];
  const recordings = data?.pronunciations.filter((sound) => sound.audioUrl) ?? [];

  return createPortal(
    <div
      ref={setPanel}
      role="dialog"
      aria-label={t('dictionary.title')}
      data-testid="word-lookup"
      style={{
        ...style,
        width: wide ? `calc(${PANEL_WIDTH}px * var(--ui-scale, 1))` : undefined,
      }}
      className={`bg-surface text-foreground motion-pop fixed z-[60] flex flex-col gap-3 overflow-y-auto overscroll-contain rounded-2xl p-4 shadow-2xl ring-1 shadow-black/40 ring-white/5 ${wide ? '' : 'max-h-[48dvh]'}`}
    >
      <header className="flex items-start gap-2">
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex flex-wrap items-baseline gap-x-2">
            <h2 className="selectable truncate text-lg font-semibold">{data?.word || query}</h2>
            {data?.languageName && <span className="text-muted text-xs">{data.languageName}</span>}
          </div>
          {(ipa.length > 0 || recordings.length > 0) && (
            <div className="flex flex-wrap items-center gap-x-1 gap-y-0.5 text-sm">
              {ipa.slice(0, 2).map((sound, index) => (
                <span key={index} className="text-muted selectable">
                  {sound.ipa}
                  {sound.label && <span className="text-xs"> ({sound.label})</span>}
                </span>
              ))}
              {recordings.slice(0, 3).map((sound) => (
                <PlayButton
                  key={sound.audioUrl}
                  url={sound.audioUrl as string}
                  label={sound.label ?? null}
                />
              ))}
            </div>
          )}
        </div>
        {asked.length > 1 && (
          <button
            type="button"
            aria-label={t('dictionary.back')}
            onClick={() => setAsked((list) => list.slice(0, -1))}
            className="hover:bg-background -my-1 grid size-9 shrink-0 place-items-center rounded-full"
          >
            <RotateCcw aria-hidden size={17} />
          </button>
        )}
        <button
          type="button"
          aria-label={t('dictionary.close')}
          onClick={onClose}
          className="hover:bg-background -my-1 -mr-2 grid size-9 shrink-0 place-items-center rounded-full"
        >
          <X aria-hidden size={19} />
        </button>
      </header>

      {data?.formOf &&
        (followed ? (
          <p className="text-muted text-sm">
            {t('dictionary.formOf', { form: data.formOf.form, note: data.formOf.note })}
          </p>
        ) : (
          <button
            type="button"
            onClick={() => lookUp(data.formOf?.lemma ?? '')}
            className="text-primary self-start text-sm hover:underline"
          >
            {t('dictionary.lemma', { lemma: data.formOf.lemma })}
          </button>
        ))}

      {!lookup && <p className="text-muted text-sm">{t('dictionary.tooLong')}</p>}
      {answer.isPending && lookup && (
        <LoaderCircle aria-label={t('dictionary.lookingUp')} className="text-muted animate-spin" />
      )}
      {answer.error && <p className="text-muted text-sm">{answer.error.message}</p>}
      {data?.offline && <OfflineNote compact />}
      {data && !found && !data.offline && (
        <p className="text-muted text-sm">
          {data.issues.length > 0
            ? data.issues.join(' · ')
            : t('dictionary.notFound', { word: data.query })}
        </p>
      )}

      {found &&
        data.meanings.map((meaning, index) => (
          <Meaning
            key={`${meaning.partOfSpeech}:${index}`}
            meaning={meaning}
            all={all}
            onLookUp={lookUp}
          />
        ))}
      {found && senses > data.meanings.length * FIRST_SENSES && (
        <button
          type="button"
          onClick={() => setAll((shown) => !shown)}
          className="text-primary self-start text-sm font-medium"
        >
          {all ? t('dictionary.showFewer') : t('dictionary.showAll', { count: senses })}
        </button>
      )}
      {found && data.etymology && (
        <details className="text-sm">
          <summary className="text-muted cursor-pointer text-xs font-semibold tracking-wide uppercase">
            {t('dictionary.origin')}
          </summary>
          <p className="selectable mt-1">{data.etymology}</p>
        </details>
      )}
      {found && data.kept && <p className="text-muted text-xs">{t('dictionary.kept')}</p>}

      <div className="flex flex-wrap gap-2">
        <ActionButton
          icon={Copy}
          onClick={() => {
            copyText(selection.text);
            showToast(t('dictionary.copied'));
          }}
        >
          {t('dictionary.copy')}
        </ActionButton>
        {lookup && (
          <ActionButton icon={Search} onClick={() => onSearchBook(query)}>
            {t('dictionary.searchBook')}
          </ActionButton>
        )}
        {onReadAloud && (
          <ActionButton icon={Volume2} onClick={onReadAloud}>
            {t('dictionary.readAloud')}
          </ActionButton>
        )}
        {data && (
          <ActionButton
            icon={ExternalLink}
            onClick={() => void commands.dictionaryOpenPage(data.onlineUrl)}
          >
            {t('dictionary.online')}
          </ActionButton>
        )}
        <TranslateRow
          key={query}
          text={query}
          from={data?.language ?? ''}
          appLanguage={appLanguage}
        />
      </div>
      {data && <Credits lookup={data} />}
    </div>,
    document.body,
  );
};
