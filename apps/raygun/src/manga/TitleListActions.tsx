import { Bookmark, Star } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { listKey } from './libraryDisplay';
import { ListSheet } from './LibrarySelection';
import type { Medium } from './medium';
import { useDuplicateCheck } from './useDuplicateCheck';
import { useSetFavorite, useSetList, useTitleMarks } from './useLibraryActions';

/**
 * A title page's list and favorite (M46): CloudStream's watch-status button and a star. Putting a
 * title on a list or among favorites adds it to the library, so it checks for duplicates first.
 */
export const TitleListActions: FC<{
  medium: Medium;
  titleId: string;
  title: string;
  pill?: boolean;
}> = ({ medium, titleId, title, pill = false }) => {
  const { t } = useTranslation();
  const marks = useTitleMarks(medium, titleId);
  const setList = useSetList(medium);
  const setFavorite = useSetFavorite(medium);
  const duplicates = useDuplicateCheck(medium);
  const [choosing, setChoosing] = useState(false);
  const adding = (run: () => void) => {
    if (marks.data?.inLibrary) run();
    else void duplicates.addChecked(titleId, title, run);
  };
  const list = marks.data?.list ?? null;
  const favorite = marks.data?.favorite ?? false;
  const video = medium === 'video';
  const listLabel = list ? t(listKey(list, video)) : t('library.lists.add');
  const tile = (pressed: boolean) =>
    pill
      ? `flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60 ${pressed ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface/80'}`
      : `flex flex-1 flex-col items-center gap-1 rounded-xl px-2 py-2 text-xs font-medium disabled:opacity-60 ${pressed ? 'text-primary' : ''}`;
  const size = pill ? 16 : 22;

  return (
    <>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-label={list ? t('library.lists.set', { name: listLabel }) : listLabel}
        disabled={!marks.data || setList.isPending || duplicates.checking}
        onClick={() => setChoosing(true)}
        className={tile(list !== null)}
      >
        <Bookmark aria-hidden size={size} fill={list ? 'currentColor' : 'none'} />
        {listLabel}
      </button>
      <button
        type="button"
        aria-pressed={favorite}
        aria-label={t(favorite ? 'library.lists.unfavorite' : 'library.lists.favorite')}
        disabled={!marks.data || setFavorite.isPending || duplicates.checking}
        onClick={() => {
          const run = () => setFavorite.mutate({ ids: [titleId], favorite: !favorite });
          if (favorite) run();
          else adding(run);
        }}
        className={tile(favorite)}
      >
        <Star aria-hidden size={size} fill={favorite ? 'currentColor' : 'none'} />
        {t('library.lists.favorite')}
      </button>
      {choosing && (
        <ListSheet
          medium={medium}
          current={list}
          onPick={(next) => {
            setChoosing(false);
            const run = () => setList.mutate({ ids: [titleId], list: next });
            if (next === null) run();
            else adding(run);
          }}
          onClose={() => setChoosing(false)}
        />
      )}
      {duplicates.dialog}
    </>
  );
};
