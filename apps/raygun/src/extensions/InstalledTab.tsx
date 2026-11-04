import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { LetterAvatar } from '../shell/LetterAvatar';
import { ListSearchField } from '../shell/ListSearchField';
import { matchesSearch } from '../shell/useListSearch';
import { KindChips } from './BrowseTab';
import { EXTENSION_KINDS, kindsOf } from './catalogFilters';
import type { ExtensionKind } from './catalogFilters';
import { useExtensions } from './useExtensions';
import { SourceIssue } from '../shell/SourceIssue';

/** Installed extensions before the list gets a search field. */
const SEARCHABLE_FROM = 8;

export const InstalledTab: FC = () => {
  const { t } = useTranslation();
  const extensions = useExtensions();
  const showNsfw = useSettings().data?.showNsfw ?? false;
  const [query, setQuery] = useState('');
  const [kind, setKind] = useState<ExtensionKind | null>(null);
  const error = extensions.error;
  const all = useMemo(() => extensions.data ?? [], [extensions.data]);
  const kinds = useMemo(() => {
    const present = new Set(all.flatMap((extension) => kindsOf(extension)));
    return EXTENSION_KINDS.filter((option) => present.has(option));
  }, [all]);
  const shown = all.filter(
    (extension) =>
      (!kind || kindsOf(extension).includes(kind)) &&
      matchesSearch(query, [extension.name, extension.description]),
  );

  return (
    <div data-testid="installed-tab" className="flex flex-col gap-3">
      {error && (
        <SourceIssue>{t('common.coreUnavailable', { message: error.message })}</SourceIssue>
      )}
      {extensions.data?.length === 0 && (
        <p className="text-muted text-sm">{t('extensions.noneInstalled')}</p>
      )}
      {all.length >= SEARCHABLE_FROM && (
        <ListSearchField
          value={query}
          onChange={setQuery}
          label={t('extensions.searchInstalled')}
          className="w-full"
        />
      )}
      <KindChips kinds={kinds} kind={kind} onChange={setKind} />
      {all.length > 0 && shown.length === 0 && (
        <p className="text-muted text-sm">{t('extensions.noMatches')}</p>
      )}
      <ul className="flex flex-col">
        {shown.map((extension) => (
          <li key={`${extension.origin}-${extension.id}`}>
            <Link
              to="/extensions/detail"
              search={{ id: extension.id }}
              data-testid="extension-row"
              className="hover:bg-surface flex items-center gap-4 rounded-xl px-1 py-2.5"
            >
              <LetterAvatar name={extension.name} logo={extension.icon} />
              <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="flex flex-wrap items-baseline gap-x-2">
                  <span className="font-medium">{extension.name}</span>
                  <span className="text-muted text-xs">{extension.version}</span>
                  <span className="bg-surface text-muted rounded px-1.5 text-[10px] font-semibold tracking-wide uppercase">
                    {t(
                      extension.origin === 'builtIn'
                        ? 'extensions.builtIn'
                        : 'extensions.installedOrigin',
                    )}
                  </span>
                  {extension.contentRating === 'nsfw' && (
                    <span className="bg-primary/20 text-primary rounded px-1.5 text-xs font-semibold">
                      {t('extensions.nsfwBadge')}
                    </span>
                  )}
                </span>
                {extension.contentRating === 'nsfw' && !showNsfw && (
                  <span className="text-muted text-xs">{t('extensions.nsfwHidden')}</span>
                )}
                {extension.status === 'failed' ? (
                  // What went wrong is on its page, under More info.
                  <span className="text-muted truncate text-xs">{t('extensions.notLoaded')}</span>
                ) : (
                  extension.description && (
                    <span className="text-muted truncate text-xs">{extension.description}</span>
                  )
                )}
              </span>
              <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
};
