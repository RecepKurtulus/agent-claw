import { useEffect, useState, useCallback } from 'react';
import {
  SpinnerIcon,
  CheckCircleIcon,
  XCircleIcon,
  ClockIcon,
  LockIcon,
  ArrowCounterClockwiseIcon,
  StopCircleIcon,
  WarningCircleIcon,
} from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import { openclawApi } from '@/shared/lib/api';
import type {
  OcRunDetail,
  OcRunTaskDetail,
  OcTaskRunStatus,
} from '../oc-types';

// ── Status badge ──────────────────────────────────────────────────────────

const STATUS_CONFIG: Record<
  OcTaskRunStatus,
  { label: string; icon: React.ReactNode; className: string }
> = {
  pending: {
    label: 'Bekliyor',
    icon: <ClockIcon className="size-3.5" weight="bold" />,
    className: 'bg-secondary text-low',
  },
  blocked: {
    label: 'Bloklu',
    icon: <LockIcon className="size-3.5" weight="bold" />,
    className: 'bg-amber-500/10 text-amber-500',
  },
  running: {
    label: 'Çalışıyor',
    icon: <SpinnerIcon className="size-3.5 animate-spin" weight="bold" />,
    className: 'bg-brand/10 text-brand',
  },
  completed: {
    label: 'Tamamlandı',
    icon: <CheckCircleIcon className="size-3.5" weight="bold" />,
    className: 'bg-emerald-500/10 text-emerald-500',
  },
  failed: {
    label: 'Başarısız',
    icon: <XCircleIcon className="size-3.5" weight="bold" />,
    className: 'bg-destructive/10 text-destructive',
  },
};

function StatusBadge({ status }: { status: OcTaskRunStatus }) {
  const cfg = STATUS_CONFIG[status] ?? STATUS_CONFIG.pending;
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium',
        cfg.className
      )}
    >
      {cfg.icon}
      {cfg.label}
    </span>
  );
}

// ── Task row ─────────────────────────────────────────────────────────────

function TaskRow({
  task,
  onRetry,
}: {
  task: OcRunTaskDetail;
  runId: string;
  onRetry: (taskId: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const hasQaError = !!task.qa_last_error;
  const hasQa = task.qa_retry_count > 0 || !!task.qa_status;

  return (
    <div
      className={cn(
        'rounded-lg border transition-colors',
        task.status === 'failed'
          ? 'border-destructive/30 bg-destructive/5'
          : task.status === 'running'
            ? 'border-brand/30 bg-brand/5'
            : task.status === 'completed'
              ? 'border-emerald-500/20 bg-emerald-500/5'
              : 'border-border bg-card'
      )}
    >
      {/* Header row */}
      <div className="flex items-center gap-3 px-4 py-3">
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium truncate">{task.task_title}</p>
          {task.task_description && (
            <p className="text-xs text-low truncate mt-0.5">
              {task.task_description}
            </p>
          )}
        </div>

        <div className="flex items-center gap-2 shrink-0">
          {/* QA badge */}
          {hasQa && (
            <span
              className={cn(
                'text-xs px-1.5 py-0.5 rounded font-mono',
                task.qa_status === 'passed'
                  ? 'bg-emerald-500/10 text-emerald-500'
                  : task.qa_status === 'failed' ||
                      task.qa_status === 'exhausted'
                    ? 'bg-destructive/10 text-destructive'
                    : 'bg-secondary text-low'
              )}
              title={`QA: ${task.qa_retry_count}/${task.qa_max_retries} deneme`}
            >
              QA {task.qa_retry_count}/{task.qa_max_retries}
            </span>
          )}

          <StatusBadge status={task.status} />

          {/* Retry button */}
          {task.status === 'failed' && (
            <button
              type="button"
              onClick={() => onRetry(task.task_id)}
              title="Yeniden Başlat"
              className="p-1 rounded hover:bg-secondary transition-colors text-low hover:text-normal"
            >
              <ArrowCounterClockwiseIcon className="size-4" />
            </button>
          )}

          {/* Expand QA error */}
          {hasQaError && (
            <button
              type="button"
              onClick={() => setExpanded((e) => !e)}
              title="Hata detayı"
              className="p-1 rounded hover:bg-secondary transition-colors text-destructive"
            >
              <WarningCircleIcon className="size-4" />
            </button>
          )}
        </div>
      </div>

      {/* QA error details */}
      {expanded && task.qa_last_error && (
        <div className="px-4 pb-3 border-t border-border/50 mt-0">
          <p className="text-xs font-medium text-destructive mb-1 pt-2">
            Son QA Hatası
          </p>
          <pre className="text-xs bg-destructive/10 rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-40 overflow-y-auto font-mono">
            {task.qa_last_error}
          </pre>
        </div>
      )}
    </div>
  );
}

// ── Run status header ─────────────────────────────────────────────────────

function RunStatusHeader({
  detail,
  onCancel,
}: {
  detail: OcRunDetail;
  onCancel: () => void;
}) {
  const doneCount = detail.tasks.filter((t) => t.status === 'completed').length;
  const total = detail.tasks.length;
  const pct = total > 0 ? Math.round((doneCount / total) * 100) : 0;

  return (
    <div className="space-y-3">
      {/* Progress bar */}
      <div>
        <div className="flex items-center justify-between mb-1">
          <span className="text-xs text-low">
            {doneCount}/{total} görev tamamlandı
          </span>
          <span className="text-xs font-mono text-low">{pct}%</span>
        </div>
        <div className="h-1.5 rounded-full bg-secondary overflow-hidden">
          <div
            className={cn(
              'h-full rounded-full transition-all duration-500',
              detail.run_status === 'completed'
                ? 'bg-emerald-500'
                : detail.run_status === 'failed' ||
                    detail.run_status === 'cancelled'
                  ? 'bg-destructive'
                  : 'bg-brand'
            )}
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>

      {/* Status row */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {detail.run_status === 'running' && (
            <>
              <SpinnerIcon className="size-4 text-brand animate-spin" />
              <span className="text-sm text-low">Çalışıyor…</span>
            </>
          )}
          {detail.run_status === 'completed' && (
            <>
              <CheckCircleIcon
                className="size-4 text-emerald-500"
                weight="bold"
              />
              <span className="text-sm text-emerald-500 font-medium">
                Tüm görevler tamamlandı!
              </span>
            </>
          )}
          {(detail.run_status === 'failed' ||
            detail.run_status === 'cancelled') && (
            <>
              <XCircleIcon className="size-4 text-destructive" weight="bold" />
              <span className="text-sm text-destructive font-medium">
                {detail.run_status === 'cancelled'
                  ? 'İptal edildi'
                  : 'Başarısız'}
              </span>
            </>
          )}
        </div>

        {/* Cancel button — only when running */}
        {detail.run_status === 'running' && (
          <button
            type="button"
            onClick={onCancel}
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs text-low hover:text-destructive hover:bg-destructive/10 transition-colors"
          >
            <StopCircleIcon className="size-3.5" />
            Durdur
          </button>
        )}
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────

interface RunMonitorPanelProps {
  runId: string;
  planId: string;
}

export function RunMonitorPanel({ runId }: RunMonitorPanelProps) {
  const [detail, setDetail] = useState<OcRunDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  const fetchDetail = useCallback(async () => {
    try {
      const d = await openclawApi.getRunDetail(runId);
      setDetail(d);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Veri yüklenemedi');
    }
  }, [runId]);

  // Initial fetch + polling
  useEffect(() => {
    fetchDetail();
    const interval = setInterval(() => {
      // Stop polling when terminal state reached
      setDetail((prev) => {
        if (
          prev &&
          (prev.run_status === 'completed' ||
            prev.run_status === 'failed' ||
            prev.run_status === 'cancelled')
        ) {
          clearInterval(interval);
          return prev;
        }
        return prev;
      });
      fetchDetail();
    }, 3000);
    return () => clearInterval(interval);
  }, [fetchDetail]);

  const handleCancel = useCallback(async () => {
    try {
      await openclawApi.cancelRun(runId);
      await fetchDetail();
    } catch {
      // ignore
    }
  }, [runId, fetchDetail]);

  const handleRetry = useCallback(
    async (taskId: string) => {
      try {
        await openclawApi.retryTask(runId, taskId);
        await fetchDetail();
      } catch {
        // ignore
      }
    },
    [runId, fetchDetail]
  );

  if (error) {
    return (
      <div className="flex items-center gap-2 text-destructive text-sm py-4">
        <XCircleIcon className="size-4" weight="bold" />
        {error}
      </div>
    );
  }

  if (!detail) {
    return (
      <div className="flex items-center gap-2 text-low text-sm py-8 justify-center">
        <SpinnerIcon className="size-4 animate-spin" />
        Yükleniyor…
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <RunStatusHeader detail={detail} onCancel={handleCancel} />

      <div className="space-y-2">
        {detail.tasks.map((task) => (
          <TaskRow
            key={task.task_id}
            task={task}
            runId={runId}
            onRetry={handleRetry}
          />
        ))}
      </div>
    </div>
  );
}
