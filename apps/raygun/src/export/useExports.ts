import { commands, events } from '@raygun/bindings';
import type { ExportJob, ExportRequest, Settings } from '@raygun/bindings';
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

export const useStartExport = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (request: ExportRequest) => unwrapResult(commands.exportStart(request)),
    onSuccess: (job) =>
      queryClient.setQueryData<ExportJob[]>(EXPORTS, (jobs) =>
        jobs?.some((known) => known.id === job.id) ? jobs : [job, ...(jobs ?? [])],
      ),
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
