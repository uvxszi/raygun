import { commands } from '@raygun/bindings';
import type { RepoCatalog } from '@raygun/bindings';
import { useQueries, useQuery } from '@tanstack/react-query';
import { Check, LoaderCircle, Plus, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { RepoContents } from './RepoContents';
import {
  useAddRepo,
  useCatalog,
  useRemoveRepo,
  useRepos,
  useSuggestedRepos,
} from './useExtensions';
import { SourceIssue } from '../shell/SourceIssue';

/** What a repository's index lists, read without adding it; cached like the catalog. */
const previewOptions = (url: string) => ({
  queryKey: ['extensionRepoPreview', url],
  queryFn: () => unwrapResult(commands.extensionRepoPreview(url)),
  staleTime: 10 * 60_000,
  retry: false,
});

/** Typing pauses this long before the address is read. */
const PREVIEW_DELAY_MS = 600;

/** Whether `text` could be a repository's address (or, on desktop, a path to an index). */
const looksLikeAddress = (text: string) =>
  /^https?:\/\/[^\s/]+\.[^\s/]+\S*$/i.test(text) ||
  /^file:\/\/\S+$/i.test(text) ||
  /^(?:[a-z]:[\\/]|\/)\S+\.json$/i.test(text);

/** What the address typed so far holds, once typing pauses on something that looks like one. */
const usePreview = (url: string) => {
  const [settled, setSettled] = useState(url.trim());
  useEffect(() => {
    const timer = setTimeout(() => setSettled(url.trim()), PREVIEW_DELAY_MS);
    return () => clearTimeout(timer);
  }, [url]);
  const wanted = looksLikeAddress(settled) && settled === url.trim() ? settled : '';
  return useQuery({ ...previewOptions(wanted), enabled: wanted !== '' });
};

/**
 * Adding a repository by its address, from the top bar's "+". What the address holds shows
 * while it's typed, before it's added.
 */
export const AddRepoSheet: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const addRepo = useAddRepo();
  const [url, setUrl] = useState('');
  const preview = usePreview(url);

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (url.trim()) {
      addRepo.mutate(url.trim(), { onSuccess: onClose });
    }
  };

  return (
    <Sheet title={t('extensions.addRepo')} onClose={onClose}>
      <form onSubmit={onSubmit} className="flex flex-col gap-3 text-sm">
        <TextField
          type="url"
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          autoFocus
          value={url}
          onChange={setUrl}
          placeholder={t('extensions.repoUrlPlaceholder')}
          aria-label={t('extensions.repoUrl')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
        />
        {preview.isFetching && (
          <p role="status" className="text-muted flex items-center gap-2 text-xs">
            <LoaderCircle aria-hidden size={14} className="animate-spin" />
            {t('extensions.reading')}
          </p>
        )}
        {preview.data && (
          <div data-testid="repo-preview" className="flex flex-col gap-1.5">
            {preview.data.repo.name && (
              <span className="font-medium">{preview.data.repo.name}</span>
            )}
            <RepoContents
              entries={preview.data.entries}
              label={t('extensions.contentsOf', { name: preview.data.repo.name })}
            />
          </div>
        )}
        <p className="text-muted text-xs">{t('extensions.repoHint')}</p>
        {addRepo.error && <SourceIssue>{addRepo.error.message}</SourceIssue>}
        <button
          type="submit"
          disabled={!url.trim() || addRepo.isPending}
          className="bg-primary text-background self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
        >
          {t('extensions.addRepo')}
        </button>
      </form>
    </Sheet>
  );
};

/**
 * Extension repositories (Mihon's): each one's address and what it holds, suggestions with what
 * they'd bring, and removal.
 */
export const ReposTab: FC = () => {
  const { t } = useTranslation();
  const repos = useRepos();
  const catalog = useCatalog();
  const addRepo = useAddRepo();
  const removeRepo = useRemoveRepo();
  const suggested = useSuggestedRepos();
  const listings = new Map<string, RepoCatalog>(
    catalog.data?.map((listing) => [listing.repo.id, listing]),
  );
  const toAdd = (suggested.data ?? []).filter((repo) => !repo.added);
  // What each suggestion holds, read from its index.
  const previews = useQueries({ queries: toAdd.map((repo) => previewOptions(repo.url)) });
  // Removing takes a second tap, so a stray one can't drop a repository.
  const [removing, setRemoving] = useState<string | null>(null);
  const onRemove = (id: string) => {
    if (removing !== id) {
      setRemoving(id);
      return;
    }
    setRemoving(null);
    removeRepo.mutate(id);
  };

  return (
    <div data-testid="repos-tab" className="flex flex-col gap-4 text-sm">
      <ul className="bg-surface flex flex-col rounded-xl">
        {repos.data?.map((repo) => {
          const listing = listings.get(repo.id);
          return (
            <li key={repo.id} data-testid="repo-row" className="flex items-center gap-3 px-4 py-3">
              <span className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="font-medium">{repo.name}</span>
                <span className="text-muted text-xs break-all">{repo.url}</span>
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
              </span>
              {removing === repo.id ? (
                <button
                  type="button"
                  // Focus moves here from the button it replaced; leaving it cancels.
                  autoFocus
                  onClick={() => onRemove(repo.id)}
                  onBlur={() => setRemoving(null)}
                  className="text-primary rounded-full px-3 py-1.5 text-sm font-semibold whitespace-nowrap"
                >
                  {t('extensions.removeRepoConfirm')}
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => onRemove(repo.id)}
                  aria-label={t('extensions.removeRepo', { name: repo.name })}
                  className="text-muted p-2"
                >
                  <Trash2 aria-hidden size={18} />
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {repos.data?.length === 0 && <p className="text-muted">{t('extensions.noReposYet')}</p>}
      {suggested.data && toAdd.length > 0 && (
        <section aria-labelledby="suggested-repos" className="flex flex-col gap-2">
          <h2 id="suggested-repos" className="text-muted font-medium">
            {t('extensions.suggestedRepos')}
          </h2>
          <ul className="bg-surface flex flex-col rounded-xl">
            {suggested.data.map((repo) => {
              const preview = previews[toAdd.indexOf(repo)]?.data;
              return (
                <li
                  key={repo.url}
                  aria-label={repo.name}
                  className="flex items-center gap-3 px-4 py-3"
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
          <p className="text-muted text-xs">{t('extensions.thirdPartyNote')}</p>
        </section>
      )}
      {(addRepo.error ?? removeRepo.error) && (
        <SourceIssue>{(addRepo.error ?? removeRepo.error)?.message}</SourceIssue>
      )}
    </div>
  );
};
