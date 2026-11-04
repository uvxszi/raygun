import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Check, Plus } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { groupWords } from '../manga/groupWords';
import type { Medium } from '../manga/medium';
import { useCategories, useCreateCategory } from '../manga/useCategories';
import { Sheet } from '../shell/Sheet';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { showToast } from '../shell/toastStore';
import { addTitles, refreshLibraries, resultMessage } from './bulkActions';
import type { PickedTitle } from './selectionStore';

/**
 * Add to group (comics, books) or folder (videos): picks one medium's groups, or makes a new one,
 * then adds the titles to the library in them. From the selection bar and from a cover's menu.
 */
export const GroupPickSheet: FC<{
  medium: Medium;
  titles: PickedTitle[];
  onDone: () => void;
  onClose: () => void;
}> = ({ medium, titles, onDone, onClose }) => {
  const { t } = useTranslation();
  const words = groupWords(medium);
  const client = useQueryClient();
  const categories = useCategories(medium);
  const create = useCreateCategory(medium);
  const [chosen, setChosen] = useState<Set<string>>(new Set());
  const [name, setName] = useState('');
  const add = useMutation({
    mutationFn: (ids: string[]) => addTitles(titles, ids),
    onSettled: () => refreshLibraries(client, [medium]),
    onSuccess: (result, ids) => {
      const names = (categories.data ?? [])
        .filter((category) => ids.includes(category.id))
        .map((category) => category.name)
        .join(', ');
      const done = result.titleIds.length;
      showToast(
        resultMessage(
          t,
          result,
          done > 0 ? t('library.categories.added', { count: done, name: names }) : null,
        ),
      );
      onDone();
    },
  });

  const toggle = (id: string) =>
    setChosen((current) => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const onCreate = (event: FormEvent) => {
    event.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    const before = new Set((categories.data ?? []).map((category) => category.id));
    create.mutate(trimmed, {
      onSuccess: async () => {
        setName('');
        // The new one is picked: it's the one the refreshed list didn't have.
        const fresh = await categories.refetch();
        const made = fresh.data?.find((category) => !before.has(category.id));
        if (made) setChosen((current) => new Set(current).add(made.id));
      },
    });
  };
  const list = categories.data ?? [];
  const error = add.error ?? create.error;

  return (
    <Sheet title={t('library.categories.addTo', words)} onClose={onClose}>
      <div className="flex flex-col gap-1 text-sm">
        {list.length === 0 && categories.data && (
          <p className="text-muted px-3">{t('library.categories.empty', words)}</p>
        )}
        {list.map((category) => {
          const on = chosen.has(category.id);
          return (
            <button
              key={category.id}
              type="button"
              role="checkbox"
              aria-checked={on}
              onClick={() => toggle(category.id)}
              className="hover:bg-background flex items-center gap-3 rounded-xl px-3 py-3 text-left"
            >
              <span
                aria-hidden
                className={`grid size-5 shrink-0 place-items-center rounded border-2 ${on ? 'border-primary bg-primary text-background' : 'border-muted'}`}
              >
                {on && <Check size={14} strokeWidth={3} />}
              </span>
              {category.name}
            </button>
          );
        })}
        <form onSubmit={onCreate} className="mt-1 flex items-center gap-2 px-1">
          <TextField
            value={name}
            onChange={setName}
            maxLength={40}
            placeholder={t('library.categories.newPlaceholder', words)}
            aria-label={t('library.categories.newName', words)}
            className={`${SHEET_FIELD} min-w-0 flex-1`}
            inputClassName={SHEET_INPUT}
          />
          <button
            type="submit"
            aria-label={t('library.categories.add', words)}
            disabled={!name.trim() || create.isPending}
            className="bg-background grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-40"
          >
            <Plus aria-hidden size={18} />
          </button>
        </form>
        {error && (
          <p role="alert" className="text-primary px-3 wrap-anywhere">
            {error.message}
          </p>
        )}
        <button
          type="button"
          disabled={chosen.size === 0 || add.isPending}
          onClick={() => add.mutate([...chosen])}
          className="bg-primary text-background mt-2 self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
        >
          {t('library.categories.addTo', words)}
        </button>
      </div>
    </Sheet>
  );
};
