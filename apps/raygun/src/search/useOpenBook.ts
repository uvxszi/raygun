import type { AudiobookResult } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';

import { useOpenAudiobook } from '../audiobooks/useAudiobooks';

/** Opens a directory's book on its page (adding it to the device, not to the library). */
export const useOpenBook = () => {
  const navigate = useNavigate();
  const open = useOpenAudiobook();
  const openBook = (book: AudiobookResult) =>
    open.mutate(book.feedUrl, {
      onSuccess: (id) => void navigate({ to: '/audiobook', search: { id } }),
    });
  return { openBook, open };
};
