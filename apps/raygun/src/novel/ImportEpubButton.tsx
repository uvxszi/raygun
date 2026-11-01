import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { BookPlus } from 'lucide-react';
import { useRef } from 'react';
import type { ChangeEvent, FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { readAsBase64 } from '../lib/readFile';
import { NOVEL } from './useNovel';

/** Imports an EPUB from the device into the novel library and opens it. */
export const ImportEpubButton: FC<{ className: string }> = ({ className }) => {
  const { t } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const importBook = useMutation({
    mutationFn: async (file: File) =>
      unwrapResult(commands.novelImportEpub(await readAsBase64(file))),
    onSuccess: (book) => {
      void queryClient.invalidateQueries({ queryKey: [NOVEL] });
      void navigate({ to: '/novel/title', search: { source: book.sourceId, id: book.novelId } });
    },
  });

  const onPick = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (file) importBook.mutate(file);
  };

  return (
    <>
      <button
        type="button"
        disabled={importBook.isPending}
        onClick={() => input.current?.click()}
        className={`${className} disabled:opacity-60`}
      >
        <BookPlus aria-hidden size={16} className="text-primary" />
        {t(importBook.isPending ? 'novel.importing' : 'novel.importEpub')}
      </button>
      <input
        ref={input}
        type="file"
        accept=".epub,application/epub+zip"
        aria-label={t('novel.epubFile')}
        onChange={onPick}
        className="hidden"
      />
      {importBook.error && (
        <p role="alert" className="text-primary w-full text-sm wrap-anywhere">
          {importBook.error.message}
        </p>
      )}
    </>
  );
};
