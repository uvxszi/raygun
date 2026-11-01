import { commands, events } from '@raygun/bindings';
import type { ExtensionInfo, SettingValue } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

export const EXTENSIONS = ['extensions'];
export const ONLINE_SEARCH = ['onlineSearch'];
const CATALOG = ['extensionCatalog'];
const REPOS = ['extensionRepos'];
const SUGGESTED = ['extensionSuggestedRepos'];

export const useExtensions = () =>
  useQuery({
    queryKey: EXTENSIONS,
    queryFn: () => unwrapResult(commands.extensionsList()),
  });

export const useExtension = (id: string) => {
  const extensions = useExtensions();
  return { ...extensions, data: extensions.data?.find((extension) => extension.id === id) };
};

/**
 * Follows extension reloads from Rust (installs, settings changes, and in dev builds every
 * `raygun-ext dev` push): refreshes the list, catalog states and search results. Mount once.
 */
export const useExtensionsSync = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.extensionsChangedEvent.listen((event) => {
      queryClient.setQueryData(EXTENSIONS, event.payload);
      void queryClient.invalidateQueries({ queryKey: ONLINE_SEARCH });
      void queryClient.invalidateQueries({ queryKey: CATALOG });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
};

/** Mutations answer with the reloaded list; store it right away rather than wait for the event. */
const useStoreExtensions = () => {
  const queryClient = useQueryClient();
  return (extensions: ExtensionInfo[]) => {
    queryClient.setQueryData(EXTENSIONS, extensions);
    void queryClient.invalidateQueries({ queryKey: CATALOG });
  };
};

export const useReloadExtensions = () => {
  const store = useStoreExtensions();
  return useMutation({
    mutationFn: () => unwrapResult(commands.extensionsReload()),
    onSuccess: store,
  });
};

export const useCatalog = () =>
  useQuery({
    queryKey: CATALOG,
    queryFn: () => unwrapResult(commands.extensionCatalog(false)),
  });

/** Re-fetches every repository index, bypassing the cache. */
export const useRefreshCatalog = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(commands.extensionCatalog(true)),
    onSuccess: (catalog) => queryClient.setQueryData(CATALOG, catalog),
  });
};

export const useInstallExtension = () => {
  const store = useStoreExtensions();
  return useMutation({
    mutationFn: ({ repoId, extensionId }: { repoId: string; extensionId: string }) =>
      unwrapResult(commands.extensionInstall(repoId, extensionId)),
    onSuccess: store,
  });
};

export const useUninstallExtension = () => {
  const store = useStoreExtensions();
  return useMutation({
    mutationFn: (extensionId: string) => unwrapResult(commands.extensionUninstall(extensionId)),
    onSuccess: store,
  });
};

export const useSetExtensionSetting = () => {
  const store = useStoreExtensions();
  return useMutation({
    mutationFn: ({
      extensionId,
      key,
      value,
    }: {
      extensionId: string;
      key: string;
      value: SettingValue;
    }) => unwrapResult(commands.extensionSettingSet(extensionId, key, value)),
    onSuccess: store,
  });
};

export const useRepos = () =>
  useQuery({
    queryKey: REPOS,
    queryFn: () => unwrapResult(commands.extensionRepos()),
  });

export const useAddRepo = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (url: string) => unwrapResult(commands.extensionRepoAdd(url)),
    onSuccess: (repos) => {
      queryClient.setQueryData(REPOS, repos);
      void queryClient.invalidateQueries({ queryKey: CATALOG });
      void queryClient.invalidateQueries({ queryKey: SUGGESTED });
    },
  });
};

export const useRemoveRepo = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrapResult(commands.extensionRepoRemove(id)),
    onSuccess: (repos) => {
      queryClient.setQueryData(REPOS, repos);
      void queryClient.invalidateQueries({ queryKey: CATALOG });
      void queryClient.invalidateQueries({ queryKey: SUGGESTED });
    },
  });
};

/** Well-known repositories to add with one tap (M36). */
export const useSuggestedRepos = () =>
  useQuery({
    queryKey: SUGGESTED,
    queryFn: () => unwrapResult(commands.extensionSuggestedRepos()),
  });
