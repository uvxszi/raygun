import { commands } from '@raygun/bindings';
import type { DictionaryPrefs, DictionarySourceInfo } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { ChevronDown, ChevronUp } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { nativeName } from '../i18n/languages';
import { unwrapResult } from '../lib/commandResult';
import { PrefChoice, PrefGroup, PrefSwitch } from '../settings/prefs';
import { useUpdateSetting } from '../settings/useSettings';
import { useDictionaryPrefs } from './prefs';

/** Languages offered for the words, besides the book's own. */
const LANGUAGES = [
  'en',
  'fr',
  'de',
  'es',
  'it',
  'pt',
  'nl',
  'sv',
  'da',
  'nb',
  'fi',
  'pl',
  'cs',
  'hu',
  'ro',
  'ru',
  'uk',
  'el',
  'tr',
  'ar',
  'he',
  'fa',
  'hi',
  'id',
  'vi',
  'ja',
  'ko',
  'zh',
  'la',
  'eo',
];

const SOURCES_KEY = ['dictionary', 'sources'] as const;

const ROW_BUTTON =
  'hover:bg-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-30';

/** One dictionary: on or off, and moved up or down the order. */
const SourceRow: FC<{
  source: DictionarySourceInfo;
  first: boolean;
  last: boolean;
  onToggle: () => void;
  onMove: (step: number) => void;
}> = ({ source, first, last, onToggle, onMove }) => {
  const { t } = useTranslation();
  const languages = source.languages.map(nativeName).join(', ');
  return (
    <li data-pref={source.name} className="flex items-center gap-2 py-1">
      <label className="flex min-w-0 flex-1 items-center gap-3 px-1 py-2">
        <input
          type="checkbox"
          checked={source.enabled}
          onChange={onToggle}
          className="accent-primary size-5 shrink-0"
        />
        <span className="flex min-w-0 flex-col">
          <span className="font-medium">{source.name}</span>
          <span className="text-muted text-sm">
            {languages
              ? t('dictionary.settings.wordsIn', { languages })
              : t('dictionary.settings.everyLanguage')}
            {!source.builtIn && ` · ${t('dictionary.settings.extension')}`}
          </span>
        </span>
      </label>
      <button
        type="button"
        aria-label={t('dictionary.settings.moveUp', { name: source.name })}
        disabled={first}
        onClick={() => onMove(-1)}
        className={ROW_BUTTON}
      >
        <ChevronUp aria-hidden size={18} />
      </button>
      <button
        type="button"
        aria-label={t('dictionary.settings.moveDown', { name: source.name })}
        disabled={last}
        onClick={() => onMove(1)}
        className={ROW_BUTTON}
      >
        <ChevronDown aria-hidden size={18} />
      </button>
    </li>
  );
};

/**
 * Settings → Reader → Dictionary: looking words up by selecting them, which dictionaries answer
 * and in what order, and the words' language when the book's isn't right.
 */
export const DictionarySettings: FC = () => {
  const { t } = useTranslation();
  const prefs = useDictionaryPrefs();
  const update = useUpdateSetting();
  const sources = useQuery({
    queryKey: [...SOURCES_KEY, prefs.order.join(','), prefs.off.join(',')],
    queryFn: () => unwrapResult(commands.dictionarySources()),
  });
  const set = (patch: Partial<DictionaryPrefs>) =>
    update.mutate({ key: 'dictionary', value: { ...prefs, ...patch } });
  const list = sources.data ?? [];
  const move = (index: number, step: number) => {
    const ids = list.map((source) => source.id);
    const [moved] = ids.splice(index, 1);
    if (moved === undefined) return;
    ids.splice(index + step, 0, moved);
    set({ order: ids });
  };
  const toggle = (source: DictionarySourceInfo) =>
    set({
      off: source.enabled ? [...prefs.off, source.id] : prefs.off.filter((id) => id !== source.id),
    });

  return (
    <PrefGroup title={t('dictionary.settings.title')} id="dictionary">
      <PrefSwitch
        title={t('dictionary.settings.enabled')}
        summary={t('dictionary.settings.enabledSummary')}
        checked={prefs.enabled}
        onChange={(enabled) => set({ enabled })}
      />
      {prefs.enabled && (
        <>
          <li data-pref={t('dictionary.settings.sources')} className="px-1 pt-3 pb-1">
            <span className="font-medium">{t('dictionary.settings.sources')}</span>
            <span className="text-muted block text-sm">
              {t('dictionary.settings.sourcesSummary')}
            </span>
          </li>
          <li>
            <ul aria-label={t('dictionary.settings.sources')} className="flex flex-col">
              {list.map((source, index) => (
                <SourceRow
                  key={source.id}
                  source={source}
                  first={index === 0}
                  last={index === list.length - 1}
                  onToggle={() => toggle(source)}
                  onMove={(step) => move(index, step)}
                />
              ))}
            </ul>
          </li>
          <PrefChoice<string>
            title={t('dictionary.settings.language')}
            summary={t('dictionary.settings.languageSummary')}
            value={prefs.language}
            options={[
              { value: '', label: t('dictionary.settings.languageBook') },
              ...LANGUAGES.map((tag) => ({ value: tag, label: nativeName(tag) })),
            ]}
            onChange={(language) => set({ language })}
          />
        </>
      )}
    </PrefGroup>
  );
};
