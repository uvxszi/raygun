import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useRef } from 'react';
import type { ChangeEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { readAsBase64 } from '../lib/readFile';
import { NOVEL } from './useNovel';

/**
 * Imports an EPUB from the device into the novel library and opens it. `pick` opens the file
 * chooser; render `input` once (it's hidden).
 */
export const useImportEpub = () => {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
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

  return {
    pick: () => inputRef.current?.click(),
    importBook,
    input: (
      <input
        ref={inputRef}
        type="file"
        accept=".epub,application/epub+zip"
        aria-label={t('novel.epubFile')}
        onChange={onPick}
        className="hidden"
      />
    ),
  };
};
