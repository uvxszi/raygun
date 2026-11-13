import { commands, events } from '@raygun/bindings';
import type { ExtensionInfo, SettingValue } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

export const EXTENSIONS = ['extensions'];
export const ONLINE_SEARCH = ['onlineSearch'];
export const CATALOG = ['extensionCatalog'];
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
 * Whether a loaded extension provides `kind` (`music.charts`, `podcast.directory`…): what a
 * screen needs a source for, so it can say there's none yet. `undefined` while the list loads.
 */
export const useHasProvider = (kind: string): boolean | undefined => {
  const extensions = useExtensions();
  return extensions.data?.some(
    (extension) =>
      extension.status === 'loaded' &&
      extension.providers.some((provider) => provider.kind === kind),
  );
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

/** Forgets an extension's cookies and stored data, then reloads it (M46.11). */
export const useClearExtensionData = () => {
  const store = useStoreExtensions();
  return useMutation({
    mutationFn: (id: string) => unwrapResult(commands.extensionClearData(id)),
    onSuccess: store,
  });
};

/** Downloads a Mihon extension's APK and checks it against its store (Allow running source code). */
export const useMihonApk = () =>
  useMutation({
    mutationFn: ({ repoId, id }: { repoId: string; id: string }) =>
      unwrapResult(commands.extensionMihonApk(repoId, id)),
  });

export const MIHON_RUNTIME = ['mihon-runtime'];

/**
 * Whether this device can run a Mihon extension's own code, kept fresh by `MihonRuntimeEvent`
 * (followed once for the app, in `shell/useLiveInvalidation`).
 */
export const useMihonRuntime = () =>
  useQuery({
    queryKey: MIHON_RUNTIME,
    queryFn: () => unwrapResult(commands.mihonRuntimeState()),
  });

/** Fetches the Mihon runtime's tools (desktop: a JRE and dex2jar). */
export const useMihonRuntimeSetup = () =>
  useMutation({ mutationFn: () => unwrapResult(commands.mihonRuntimeSetup()) });

/** Stops fetching the Mihon runtime's tools at once; nothing of the download is kept. */
export const useMihonRuntimeCancel = () =>
  useMutation({ mutationFn: () => unwrapResult(commands.mihonRuntimeCancel()) });

/** Runs one call on an installed, checked Mihon APK through the container; the result is JSON. */
export const useMihonRun = () =>
  useMutation({
    mutationFn: ({ pkg, method, params }: { pkg: string; method: string; params: unknown }) =>
      unwrapResult(commands.mihonRun(pkg, method, JSON.stringify(params ?? {}))),
  });

/** Opens the site an extension's source reads in Raygun's browser (M46.11). */
export const useOpenExtensionWebsite = () =>
  useMutation({ mutationFn: (id: string) => unwrapResult(commands.extensionOpenWebsite(id)) });

export const useReloadExtensions = () => {
  const store = useStoreExtensions();
  return useMutation({
    mutationFn: () => unwrapResult(commands.extensionsReload()),
    onSuccess: store,
  });
};

/**
 * Every repository's extensions. Whatever changes it (installs, repositories, reloads, 18+
 * marks) reads it again; otherwise a large catalogue (thousands of entries) isn't asked for again
 * each time a screen showing it opens, such as switching to the Extensions tab and back.
 */
export const useCatalog = (enabled = true) =>
  useQuery({
    queryKey: CATALOG,
    queryFn: () => unwrapResult(commands.extensionCatalog(false)),
    enabled,
    staleTime: 60_000,
  });

/**
 * The catalogue as last read, without reading it again while there's one (Search's Custom
 * sources groups by the languages it lists): read only when nothing is kept.
 */
export const useKnownCatalog = (enabled: boolean) =>
  useQuery({
    queryKey: CATALOG,
    queryFn: () => unwrapResult(commands.extensionCatalog(false)),
    enabled,
    staleTime: Infinity,
  });

/** Re-fetches every repository index, bypassing the cache. */
export const useRefreshCatalog = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(commands.extensionCatalog(true)),
    onSuccess: (catalog) => queryClient.setQueryData(CATALOG, catalog),
  });
};

/** Fetches one repository's index again (its Reload); answers with the whole catalogue. */
export const useRefreshRepo = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (repoId: string) => unwrapResult(commands.extensionCatalogRefreshRepo(repoId)),
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
      // Installed extensions name the repository they came from.
      void queryClient.invalidateQueries({ queryKey: EXTENSIONS });
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
      // Its extensions stay, now added by you.
      void queryClient.invalidateQueries({ queryKey: EXTENSIONS });
    },
  });
};

/** Well-known repositories to add with one tap (M36). */
export const useSuggestedRepos = () =>
  useQuery({
    queryKey: SUGGESTED,
    queryFn: () => unwrapResult(commands.extensionSuggestedRepos()),
  });
