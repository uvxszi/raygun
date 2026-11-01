import { commands, events } from '@raygun/bindings';
import type { AiModel, AiProviderInput } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const PROVIDERS = ['ai', 'providers'];
const MODELS = ['ai', 'models'];

export const useAiProviders = () =>
  useQuery({ queryKey: PROVIDERS, queryFn: () => unwrapResult(commands.aiProviders()) });

export const useSaveAiProvider = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: AiProviderInput) => unwrapResult(commands.aiProviderSave(input)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: PROVIDERS }),
  });
};

export const useDeleteAiProvider = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrapResult(commands.aiProviderDelete(id)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: PROVIDERS }),
  });
};

export const useTestAiProvider = () =>
  useMutation({ mutationFn: (id: string) => unwrapResult(commands.aiProviderTest(id)) });

const setDownloading = (list: AiModel[] | undefined, id: string, received: number | null) =>
  list?.map((model) => (model.id === id ? { ...model, downloading: received } : model));

/** The model catalogue, following download progress from Rust. */
export const useAiModels = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.aiModelEvent.listen(({ payload }) => {
      if (payload.state === 'downloading') {
        queryClient.setQueryData<AiModel[]>(MODELS, (list) =>
          setDownloading(list, payload.id, payload.receivedBytes),
        );
      } else {
        void queryClient.invalidateQueries({ queryKey: MODELS });
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
  return useQuery({ queryKey: MODELS, queryFn: () => commands.aiModels() });
};

export const useDownloadAiModel = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrapResult(commands.aiModelDownload(id)),
    onMutate: (id) => {
      queryClient.setQueryData<AiModel[]>(MODELS, (list) => setDownloading(list, id, 0));
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: MODELS }),
  });
};

export const useDeleteAiModel = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrapResult(commands.aiModelDelete(id)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: MODELS }),
  });
};
