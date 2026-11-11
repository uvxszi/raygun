import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useRef } from 'react';
import type { ChangeEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { uploadFile } from '../manga/useImportComics';
import { NOVEL } from './useNovel';

/**
 * Imports a book (EPUB or PDF) from the device into the novel library and opens it. `pick` opens
 * the file chooser; render `input` once (it's hidden). The file goes to Rust in slices, so a big
 * book never has to fit in one message.
 */
export const useImportEpub = () => {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const importBook = useMutation({
    mutationFn: async (file: File) =>
      unwrapResult(commands.novelImportEpubUpload(await uploadFile(file), file.name)),
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
        accept=".epub,application/epub+zip,.pdf,application/pdf"
        aria-label={t('novel.epubFile')}
        onChange={onPick}
        className="hidden"
      />
    ),
  };
};
