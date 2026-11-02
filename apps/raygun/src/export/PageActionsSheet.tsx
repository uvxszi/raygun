import { commands } from '@raygun/bindings';
import type { PageRef } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { Copy, Download, FolderOpen, Share2 } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { usePlatform } from '../lib/usePlatform';
import { Sheet } from '../shell/Sheet';

type PageAction = 'save' | 'share' | 'copy';

const run = (action: PageAction, page: PageRef) =>
  action === 'save'
    ? unwrapResult(commands.pageSave(page))
    : unwrapResult(action === 'share' ? commands.pageShare(page) : commands.pageCopy(page)).then(
        () => null,
      );

/** A long press on a reader page (Mihon's page actions): save it to Pictures, share or copy it. */
export const PageActionsSheet: FC<{ page: PageRef; onClose: () => void }> = ({ page, onClose }) => {
  const { t } = useTranslation();
  // Desktop has no share sheet: the file is shown in the file manager instead (M46.6).
  const desktop = (usePlatform() ?? 'android') !== 'android';
  const act = useMutation({
    mutationFn: (action: PageAction) => run(action, page),
    onSuccess: (_saved, action) => {
      if (action === 'share') onClose();
    },
  });
  const done = act.isSuccess
    ? act.variables === 'save'
      ? t('reader.savedTo', { place: act.data })
      : act.variables === 'copy'
        ? t('reader.copied')
        : null
    : null;
  const action = (kind: PageAction, label: string, Icon: typeof Download) => (
    <button
      type="button"
      disabled={act.isPending}
      onClick={() => act.mutate(kind)}
      className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-3 text-left disabled:opacity-50"
    >
      <Icon aria-hidden size={20} className="text-muted" />
      {label}
    </button>
  );
  return (
    <Sheet title={t('reader.pageTitle', { page: page.index + 1 })} onClose={onClose}>
      <div className="-mx-2 flex flex-col text-sm">
        {action('save', t('reader.savePage'), Download)}
        {desktop
          ? action('share', t('reader.showInFolder'), FolderOpen)
          : action('share', t('reader.sharePage'), Share2)}
        {action('copy', t('reader.copyPage'), Copy)}
      </div>
      {done && (
        <p role="status" className="text-muted text-sm wrap-anywhere">
          {done}
        </p>
      )}
      {act.isError && (
        <p role="alert" className="text-primary text-sm">
          {act.error.message}
        </p>
      )}
    </Sheet>
  );
};
