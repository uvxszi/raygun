import { commands } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

const WEBSITES = ['websites'] as const;

export const useWebsites = () =>
  useQuery({
    queryKey: WEBSITES,
    queryFn: () => unwrapResult(commands.websiteList()),
  });

const useWebsiteMutation = <Args, Result>(run: (args: Args) => Promise<Result>) => {
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

/** A site's own ad blocker, as its menu in the browser sets it. */
export const useSetWebsiteBlocking = () =>
  useWebsiteMutation(({ id, on }: { id: string; on: boolean }) =>
    unwrapResult(commands.websiteSetBlockAds(id, on)),
  );

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

const SITE_SCRIPTS = [...WEBSITES, 'siteScripts'] as const;

/** Installed site scripts and the websites they run on (Settings → Browse → Websites). */
export const useSiteScripts = () =>
  useQuery({
    queryKey: SITE_SCRIPTS,
    queryFn: () => unwrapResult(commands.websiteSiteScripts()),
  });

/** One site's switch for an extension's site scripts; applies when the site next opens. */
export const useSetSiteScripts = () =>
  useWebsiteMutation(
    ({ siteId, extensionId, on }: { siteId: string; extensionId: string; on: boolean }) =>
      unwrapResult(commands.websiteSetSiteScripts(siteId, extensionId, on)),
  );
