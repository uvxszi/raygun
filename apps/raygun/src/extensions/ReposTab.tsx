import { Check, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { useAddRepo, useRemoveRepo, useRepos, useSuggestedRepos } from './useExtensions';
import { SourceIssue } from '../shell/SourceIssue';

/** Adding a repository by its address, from the top bar's "+". */
export const AddRepoSheet: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const addRepo = useAddRepo();
  const [url, setUrl] = useState('');

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (url.trim()) {
      addRepo.mutate(url.trim(), { onSuccess: onClose });
    }
  };

  return (
    <Sheet title={t('extensions.addRepo')} onClose={onClose}>
      <form onSubmit={onSubmit} className="flex flex-col gap-3 text-sm">
        <input
          type="url"
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          autoFocus
          value={url}
          onChange={(event) => setUrl(event.target.value)}
          placeholder={t('extensions.repoUrlPlaceholder')}
          aria-label={t('extensions.repoUrl')}
          className="bg-background placeholder:text-muted rounded-xl px-4 py-3 outline-none"
        />
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

export const ReposTab: FC = () => {
  const { t } = useTranslation();
  const repos = useRepos();
  const addRepo = useAddRepo();
  const removeRepo = useRemoveRepo();
  const suggested = useSuggestedRepos();

  return (
    <div data-testid="repos-tab" className="flex flex-col gap-4 text-sm">
      <ul className="bg-surface flex flex-col rounded-xl">
        {repos.data?.map((repo) => (
          <li key={repo.id} data-testid="repo-row" className="flex items-center gap-3 px-4 py-3">
            <span className="flex min-w-0 flex-1 flex-col">
              <span className="font-medium">{repo.name}</span>
              <span className="text-muted text-xs break-all">{repo.url}</span>
            </span>
            <button
              type="button"
              onClick={() => removeRepo.mutate(repo.id)}
              aria-label={t('extensions.removeRepo', { name: repo.name })}
              className="text-muted p-2"
            >
              <Trash2 aria-hidden size={18} />
            </button>
          </li>
        ))}
      </ul>
      {repos.data?.length === 0 && <p className="text-muted">{t('extensions.noRepos')}</p>}
      {suggested.data && suggested.data.some((repo) => !repo.added) && (
        <section aria-labelledby="suggested-repos" className="flex flex-col gap-2">
          <h2 id="suggested-repos" className="text-muted font-medium">
            {t('extensions.suggestedRepos')}
          </h2>
          <ul className="bg-surface flex flex-col rounded-xl">
            {suggested.data.map((repo) => (
              <li
                key={repo.url}
                aria-label={repo.name}
                className="flex items-center gap-3 px-4 py-3"
              >
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="font-medium">{repo.name}</span>
                  <span className="text-muted text-xs">{repo.description}</span>
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
            ))}
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
