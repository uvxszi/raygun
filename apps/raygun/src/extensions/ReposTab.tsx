import { Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useAddRepo, useRemoveRepo, useRepos } from './useExtensions';

export const ReposTab: FC = () => {
  const { t } = useTranslation();
  const repos = useRepos();
  const addRepo = useAddRepo();
  const removeRepo = useRemoveRepo();
  const [url, setUrl] = useState('');

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (url.trim()) {
      addRepo.mutate(url.trim(), { onSuccess: () => setUrl('') });
    }
  };

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
      <form onSubmit={onSubmit} className="flex flex-col gap-2">
        <div className="flex gap-2">
          <input
            type="url"
            inputMode="url"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            placeholder={t('extensions.repoUrlPlaceholder')}
            aria-label={t('extensions.repoUrl')}
            className="bg-surface placeholder:text-muted min-w-0 flex-1 rounded-lg px-3 py-2 outline-none"
          />
          <button
            type="submit"
            disabled={!url.trim() || addRepo.isPending}
            className="bg-surface rounded-lg px-4 py-2 disabled:opacity-40"
          >
            {t('extensions.addRepo')}
          </button>
        </div>
        <p className="text-muted text-xs">{t('extensions.repoHint')}</p>
        {(addRepo.error ?? removeRepo.error) && (
          <p role="alert" className="text-primary wrap-anywhere">
            {(addRepo.error ?? removeRepo.error)?.message}
          </p>
        )}
      </form>
    </div>
  );
};
