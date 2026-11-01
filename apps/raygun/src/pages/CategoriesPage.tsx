import { useSearch } from '@tanstack/react-router';
import { ArrowDown, ArrowUp, Check, Pencil, Plus, Trash2, X } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import type { Medium } from '../manga/medium';
import {
  useCategories,
  useCreateCategory,
  useDeleteCategory,
  useRenameCategory,
  useReorderCategories,
} from '../manga/useCategories';
import { Sheet } from '../shell/Sheet';
import { TopBarAction } from '../shell/TopBar';
import { PageHeader } from './PageHeader';

const ICON =
  'bg-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-40';

/** Library → Edit categories: add, rename, reorder and delete a library's categories. */
export const CategoriesPage: FC = () => {
  const { t } = useTranslation();
  const { medium } = useSearch({ from: '/library/categories' }) as { medium: Medium };
  const categories = useCategories(medium);
  const create = useCreateCategory(medium);
  const rename = useRenameCategory(medium);
  const remove = useDeleteCategory(medium);
  const reorder = useReorderCategories(medium);
  const [name, setName] = useState('');
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);
  const list = categories.data ?? [];

  const onCreate = (event: FormEvent) => {
    event.preventDefault();
    if (name.trim()) {
      create.mutate(name.trim(), {
        onSuccess: () => {
          setName('');
          setCreating(false);
        },
      });
    }
  };
  const onRename = (event: FormEvent) => {
    event.preventDefault();
    if (editing?.name.trim()) {
      rename.mutate(
        { id: editing.id, name: editing.name.trim() },
        { onSuccess: () => setEditing(null) },
      );
    }
  };
  const move = (index: number, step: number) => {
    const ids = list.map((item) => item.id);
    const [moved] = ids.splice(index, 1);
    ids.splice(index + step, 0, moved!);
    reorder.mutate(ids);
  };
  const error = create.error ?? rename.error ?? remove.error ?? reorder.error;

  return (
    <section data-testid="categories-page" className="flex max-w-2xl flex-col gap-4">
      <PageHeader
        back
        fallback="/library"
        title={t(`library.categories.${medium}Title`)}
        actions={
          <TopBarAction
            label={t('library.categories.add')}
            icon={Plus}
            onClick={() => setCreating(true)}
          />
        }
      />
      <p className="text-muted text-sm">{t('library.categories.hint')}</p>
      {list.length === 0 && categories.data && (
        <p className="text-sm">{t('library.categories.empty')}</p>
      )}
      <ul className="bg-surface flex flex-col rounded-xl">
        {list.map((item, index) => (
          <li key={item.id} aria-label={item.name} className="flex items-center gap-2 px-4 py-3">
            {editing?.id === item.id ? (
              <form onSubmit={onRename} className="flex min-w-0 flex-1 gap-2">
                <input
                  value={editing.name}
                  onChange={(event) => setEditing({ id: item.id, name: event.target.value })}
                  aria-label={t('library.categories.name')}
                  maxLength={40}
                  className="bg-background min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none"
                />
                <button type="submit" aria-label={t('library.categories.save')} className={ICON}>
                  <Check aria-hidden size={16} />
                </button>
                <button
                  type="button"
                  aria-label={t('library.categories.cancel')}
                  onClick={() => setEditing(null)}
                  className={ICON}
                >
                  <X aria-hidden size={16} />
                </button>
              </form>
            ) : (
              <>
                <span className="min-w-0 flex-1 truncate font-medium">{item.name}</span>
                <button
                  type="button"
                  aria-label={t('library.categories.moveUp', { name: item.name })}
                  disabled={index === 0}
                  onClick={() => move(index, -1)}
                  className={ICON}
                >
                  <ArrowUp aria-hidden size={16} />
                </button>
                <button
                  type="button"
                  aria-label={t('library.categories.moveDown', { name: item.name })}
                  disabled={index === list.length - 1}
                  onClick={() => move(index, 1)}
                  className={ICON}
                >
                  <ArrowDown aria-hidden size={16} />
                </button>
                <button
                  type="button"
                  aria-label={t('library.categories.rename', { name: item.name })}
                  onClick={() => setEditing({ id: item.id, name: item.name })}
                  className={ICON}
                >
                  <Pencil aria-hidden size={16} />
                </button>
                <button
                  type="button"
                  aria-label={t('library.categories.delete', { name: item.name })}
                  onClick={() => remove.mutate(item.id)}
                  className={ICON}
                >
                  <Trash2 aria-hidden size={16} />
                </button>
              </>
            )}
          </li>
        ))}
      </ul>
      {error && !creating && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {error.message}
        </p>
      )}
      {creating && (
        <Sheet title={t('library.categories.add')} onClose={() => setCreating(false)}>
          <form onSubmit={onCreate} className="flex flex-col gap-3 text-sm">
            <input
              autoFocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder={t('library.categories.newPlaceholder')}
              aria-label={t('library.categories.newName')}
              maxLength={40}
              className="bg-background placeholder:text-muted rounded-xl px-4 py-3 outline-none"
            />
            {create.error && (
              <p role="alert" className="text-primary wrap-anywhere">
                {create.error.message}
              </p>
            )}
            <button
              type="submit"
              disabled={!name.trim() || create.isPending}
              className="bg-primary text-background flex items-center gap-1.5 self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
            >
              <Plus aria-hidden size={16} />
              {t('library.categories.add')}
            </button>
          </form>
        </Sheet>
      )}
    </section>
  );
};
