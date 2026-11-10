import { commands } from '@raygun/bindings';
import type { ExtensionRepo, RepoCatalog } from '@raygun/bindings';
import { useQueries } from '@tanstack/react-query';
import { Check, Plus, Trash2 } from 'lucide-react';
import { memo, useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { AdultMark } from '../shell/AdultMark';
import { OverflowMenu } from '../shell/OverflowMenu';
import { SourceIssue } from '../shell/SourceIssue';
import { rowsEqual } from '../shell/windowedRows';
import type { RowProps } from '../shell/windowedRows';
import { repoFormat } from './catalogFilters';
import { useContentRating } from './contentRating';
import { RatingHint } from './RatingHint';
import { RepoContents } from './RepoContents';
import { useAddRepo, useRemoveRepo, useSuggestedRepos } from './useExtensions';

/** What a repository's index lists, read without adding it; cached like the catalog. */
const previewOptions = (url: string) => ({
  queryKey: ['extensionRepoPreview', url],
  queryFn: () => unwrapResult(commands.extensionRepoPreview(url)),
  staleTime: 10 * 60_000,
  retry: false,
});

/**
 * An added repository (Mihon's repositories, in Sources): its name and address, which app's
 * format it is, what it holds, why it couldn't be read, and removal, which takes a second tap so
 * a stray one can't drop it.
 */
export const RepoRow = memo<{
  repo: ExtensionRepo;
  listing: RepoCatalog | undefined;
  row: RowProps;
}>(({ repo, listing, row }) => {
  const { t } = useTranslation();
  const removeRepo = useRemoveRepo();
  const rating = useContentRating();
  const [removing, setRemoving] = useState(false);
  const mark = rating.markOf('repositories', repo.id);
  // A store's thousands of entries are read once, not each time the page is drawn.
  const format = useMemo(
    () => (listing && !listing.error ? repoFormat(listing.entries) : null),
    [listing],
  );
  return (
    <li
      {...row}
      data-testid="repo-row"
      aria-label={repo.name}
      className="bg-surface flex items-center gap-3 rounded-xl px-4 py-3"
    >
      <span className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="flex flex-wrap items-center gap-x-2">
          <span className="font-medium">{repo.name}</span>
          {mark === 'adult' && <AdultMark />}
          <RatingHint mark={mark} />
        </span>
        <span className="text-muted text-xs break-all">
          {format ? `${t(`sources.repoKind.${format}`)} · ${repo.url}` : repo.url}
        </span>
        {listing && !listing.error && (
          <RepoContents
            entries={listing.entries}
            label={t('extensions.contentsOf', { name: repo.name })}
          />
        )}
        {listing?.error && (
          <SourceIssue compact source={repo.name}>
            {t('extensions.repoFailed', { message: listing.error })}
          </SourceIssue>
        )}
        {removeRepo.error && <SourceIssue compact>{removeRepo.error.message}</SourceIssue>}
      </span>
      {removing ? (
        <button
          type="button"
          // Focus moves here from the button it replaced; leaving it cancels.
          autoFocus
          onClick={() => {
            setRemoving(false);
            removeRepo.mutate(repo.id);
          }}
          onBlur={() => setRemoving(false)}
          className="text-primary rounded-full px-3 py-1.5 text-sm font-semibold whitespace-nowrap"
        >
          {t('extensions.removeRepoConfirm')}
        </button>
      ) : (
        <button
          type="button"
          onClick={() => setRemoving(true)}
          aria-label={t('extensions.removeRepo', { name: repo.name })}
          className="text-muted p-2"
        >
          <Trash2 aria-hidden size={18} />
        </button>
      )}
      <OverflowMenu
        label={t('extensions.repoActions', { name: repo.name })}
        items={rating.menuItems('repositories', repo.id)}
      />
    </li>
  );
}, rowsEqual);

/** Well-known repositories not added yet, with what each would bring, added with one tap. */
export const SuggestedRepos: FC = () => {
  const { t } = useTranslation();
  const suggested = useSuggestedRepos();
  const addRepo = useAddRepo();
  const toAdd = (suggested.data ?? []).filter((repo) => !repo.added);
  // What each suggestion holds, read from its index.
  const previews = useQueries({ queries: toAdd.map((repo) => previewOptions(repo.url)) });
  if (!suggested.data || toAdd.length === 0) return null;
  return (
    <section aria-labelledby="suggested-repos" className="flex flex-col gap-2 text-sm">
      <h3 id="suggested-repos" className="text-muted px-1 font-medium">
        {t('extensions.suggestedRepos')}
      </h3>
      <ul className="flex flex-col gap-2">
        {suggested.data.map((repo) => {
          const preview = previews[toAdd.indexOf(repo)]?.data;
          return (
            <li
              key={repo.url}
              aria-label={repo.name}
              className="bg-surface flex items-center gap-3 rounded-xl px-4 py-3"
            >
              <span className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="font-medium">{repo.name}</span>
                {preview && preview.entries.length > 0 ? (
                  <RepoContents
                    entries={preview.entries}
                    label={t('extensions.contentsOf', { name: repo.name })}
                  />
                ) : (
                  <span className="text-muted text-xs">{repo.description}</span>
                )}
              </span>
              {repo.added ? (
                <span className="text-muted flex items-center gap-1 text-xs">
                  <Check aria-hidden size={14} />
                  {t('extensions.repoAdded')}
                </span>
              ) : (
                <button
                  type="button"
                  disabled={addRepo.isPending}
                  onClick={() => addRepo.mutate(repo.url)}
                  aria-label={t('extensions.addNamedRepo', { name: repo.name })}
                  className="bg-primary text-background flex items-center gap-1 rounded-full px-3 py-1.5 text-sm font-semibold disabled:opacity-60"
                >
                  <Plus aria-hidden size={14} />
                  {t('extensions.add')}
                </button>
              )}
            </li>
          );
        })}
      </ul>
      <p className="text-muted px-1 text-xs">{t('extensions.thirdPartyNote')}</p>
      {addRepo.error && <SourceIssue>{addRepo.error.message}</SourceIssue>}
    </section>
  );
};
