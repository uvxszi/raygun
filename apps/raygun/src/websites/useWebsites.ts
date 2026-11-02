import { commands } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

const WEBSITES = ['websites'] as const;

export const useWebsites = () =>
  useQuery({
    queryKey: WEBSITES,
    queryFn: () => unwrapResult(commands.websiteList()),
  });

const useWebsiteMutation = <Args>(run: (args: Args) => Promise<unknown>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: WEBSITES }),
  });
};

export const useAddWebsite = () =>
  useWebsiteMutation(({ name, url }: { name: string | null; url: string }) =>
    unwrapResult(commands.websiteAdd(name, url)),
  );

export const useEditWebsite = () =>
  useWebsiteMutation(({ id, name, url }: { id: string; name: string; url: string }) =>
    unwrapResult(commands.websiteEdit(id, name, url)),
  );

export const useRemoveWebsite = () =>
  useWebsiteMutation((id: string) => unwrapResult(commands.websiteRemove(id)));

export const useResetWebsite = () =>
  useWebsiteMutation((id: string) => unwrapResult(commands.websiteReset(id)));

export const useOpenWebsite = () =>
  useMutation({ mutationFn: (id: string) => unwrapResult(commands.websiteOpen(id)) });

export const useBlocklist = () =>
  useQuery({
    queryKey: [...WEBSITES, 'blocklist'],
    queryFn: () => unwrapResult(commands.websiteBlocklistStatus()),
  });

export const useUpdateBlocklist = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(commands.websiteBlocklistUpdate()),
    onSettled: () => void queryClient.invalidateQueries({ queryKey: [...WEBSITES, 'blocklist'] }),
  });
};

export const useClearWebsiteData = () =>
  useMutation({ mutationFn: () => unwrapResult(commands.websiteClearData()) });
