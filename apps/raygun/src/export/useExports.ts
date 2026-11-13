import { commands, events } from '@raygun/bindings';
import type { ExportFolder, ExportJob, ExportRequest, Settings } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const EXPORTS = ['exports'] as const;

/** This session's exports, kept current by `ExportEvent`s. */
export const useExportJobs = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.exportEvent.listen(({ payload }) => {
      if (!payload) {
        void queryClient.invalidateQueries({ queryKey: EXPORTS });
        return;
      }
      queryClient.setQueryData<ExportJob[]>(EXPORTS, (jobs) => {
        if (!jobs) return [payload];
        const index = jobs.findIndex((job) => job.id === payload.id);
        if (index < 0) return [payload, ...jobs];
        return jobs.map((job) => (job.id === payload.id ? payload : job));
      });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
  return useQuery({
    queryKey: EXPORTS,
    queryFn: () => commands.exportJobs(),
  });
};

/** What an export would do, worked out again whenever the request changes. */
export const useExportPlan = (request: ExportRequest | null) =>
  useQuery({
    queryKey: [...EXPORTS, 'plan', request],
    queryFn: () => unwrapResult(commands.exportPlan(request as ExportRequest)),
    enabled: request !== null,
  });

/**
 * The sources a video title's episodes can download from (its servers or addons), best first,
 * with the one to preselect: the one played last, else the best ranked. Asked only for one title.
 */
export const useExportVideoSources = (titleId: string | null, unitId: string | null) =>
  useQuery({
    queryKey: [...EXPORTS, 'sources', titleId, unitId],
    queryFn: () => unwrapResult(commands.exportVideoSources(titleId as string, unitId)),
    enabled: titleId !== null,
    staleTime: 60_000,
  });

const FFMPEG_STATUS = ['ytdlp', 'status'] as const;

/**
 * FFmpeg for re-encoding videos on a computer (fetched as for YouTube downloads), followed while
 * `wanted`: its fetch reports progress, and the plan is worked out again once it's there.
 */
export const useExportFfmpeg = (wanted: boolean) => {
  const client = useQueryClient();
  useEffect(() => {
    if (!wanted) return;
    const unlisten = events.ytDlpEvent.listen(({ payload }) => {
      if (!payload) void client.invalidateQueries({ queryKey: FFMPEG_STATUS });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client, wanted]);
  const status = useQuery({
    queryKey: FFMPEG_STATUS,
    queryFn: () => unwrapResult(commands.ytdlpStatus()),
    enabled: wanted,
  });
  const install = useMutation({
    mutationFn: () => unwrapResult(commands.ytdlpInstallFfmpeg()),
    onSettled: () => {
      void client.invalidateQueries({ queryKey: FFMPEG_STATUS });
      void client.invalidateQueries({ queryKey: [...EXPORTS, 'plan'] });
    },
  });
  // Stops the fetch at once; nothing of it is kept, and Get FFmpeg is offered again.
  const cancel = useMutation({
    mutationFn: () => commands.ytdlpCancelInstall(),
    onSettled: () => void client.invalidateQueries({ queryKey: FFMPEG_STATUS }),
  });
  return { status, install, cancel };
};

const LAST_FOLDER = [...EXPORTS, 'lastFolder'] as const;

/** The folder last chosen in the export sheet, offered there again. */
export const useLastExportFolder = () =>
  useQuery({
    queryKey: LAST_FOLDER,
    queryFn: () => unwrapResult(commands.exportLastFolder()),
  });

/** The format last exported to for a medium (`null` before the first export), offered first. */
export const useLastExportFormat = (medium: string) =>
  useQuery({
    queryKey: [...EXPORTS, 'lastFormat', medium],
    queryFn: () => unwrapResult(commands.exportLastFormat(medium)),
  });

/** The system folder picker for one export; `null` when it was closed without a choice. */
export const usePickExportFolder = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(commands.exportPickFolder()),
    onSuccess: (folder) => {
      if (folder) queryClient.setQueryData<ExportFolder | null>(LAST_FOLDER, folder);
    },
  });
};

export const useStartExport = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (request: ExportRequest) => unwrapResult(commands.exportStart(request)),
    onSuccess: (job, request) => {
      queryClient.setQueryData<ExportJob[]>(EXPORTS, (jobs) =>
        jobs?.some((known) => known.id === job.id) ? jobs : [job, ...(jobs ?? [])],
      );
      // Rust remembers the format: the next sheet offers it first.
      queryClient.setQueryData([...EXPORTS, 'lastFormat', request.medium], request.format);
    },
  });
};

export const useCancelExport = () =>
  useMutation({ mutationFn: (id: string) => commands.exportCancel(id) });

export const useDismissExport = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => commands.exportDismiss(id),
    onSuccess: (_result, id) =>
      queryClient.setQueryData<ExportJob[]>(EXPORTS, (jobs) =>
        jobs?.filter((job) => job.id !== id),
      ),
  });
};

/** The system folder picker; the chosen folder becomes where downloads are saved. */
export const usePickFolder = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(commands.savePickFolder()),
    onSuccess: (settings: Settings) => queryClient.setQueryData(['settings'], settings),
  });
};

/** Whether Raygun may still write to the picked folder. */
export const useFolderState = (uri: string | null) =>
  useQuery({
    queryKey: [...EXPORTS, 'folder', uri],
    queryFn: () => unwrapResult(commands.saveFolderState(uri as string)),
    enabled: uri !== null,
  });
