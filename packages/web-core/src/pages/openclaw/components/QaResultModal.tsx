import { useState, useEffect, useCallback } from 'react';
import {
  XIcon,
  CheckCircleIcon,
  ArrowCounterClockwiseIcon,
  WrenchIcon,
  SpinnerIcon,
  TerminalIcon,
  CopyIcon,
  CheckIcon,
} from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import { openclawApi } from '@/shared/lib/api';
import type { OcQaDetail, OcQaResult } from '../oc-types';

// ── Output line colorizer ─────────────────────────────────────────────────

type LineKind = 'error' | 'pass' | 'warning' | 'info' | 'normal';

function classifyLine(line: string): LineKind {
  const lower = line.toLowerCase();
  if (
    lower.includes('error') ||
    lower.includes('fail') ||
    lower.includes('panicked') ||
    lower.includes('✗') ||
    lower.includes('× ') ||
    lower.startsWith('e ') ||
    lower.startsWith('failed')
  )
    return 'error';
  if (
    lower.includes('ok') ||
    lower.includes('pass') ||
    lower.includes('✓') ||
    lower.startsWith('test result: ok')
  )
    return 'pass';
  if (lower.includes('warn')) return 'warning';
  if (lower.startsWith('running') || lower.startsWith('compiling'))
    return 'info';
  return 'normal';
}

const LINE_COLORS: Record<LineKind, string> = {
  error: 'text-red-400',
  pass: 'text-emerald-400',
  warning: 'text-amber-400',
  info: 'text-blue-400',
  normal: 'text-slate-300',
};

function ColorizedOutput({ output }: { output: string }) {
  const lines = output.split('\n');
  return (
    <pre className="text-xs font-mono leading-5 whitespace-pre-wrap break-all">
      {lines.map((line, i) => (
        <span key={i} className={cn('block', LINE_COLORS[classifyLine(line)])}>
          {line || '\u200B'}
        </span>
      ))}
    </pre>
  );
}

// ── Attempt tabs ──────────────────────────────────────────────────────────

function AttemptTabs({
  results,
  activeIdx,
  onSelect,
}: {
  results: OcQaResult[];
  activeIdx: number;
  onSelect: (i: number) => void;
}) {
  return (
    <div className="flex gap-1 border-b border-border pb-0 overflow-x-auto">
      {results.map((r, i) => (
        <button
          key={r.id}
          type="button"
          onClick={() => onSelect(i)}
          className={cn(
            'px-3 py-1.5 text-xs font-medium rounded-t shrink-0 transition-colors',
            activeIdx === i
              ? 'bg-card border border-border border-b-card text-normal -mb-px'
              : 'text-low hover:text-normal hover:bg-secondary'
          )}
        >
          {r.passed ? (
            <span className="flex items-center gap-1">
              <CheckCircleIcon className="size-3 text-emerald-500" />
              Deneme #{r.attempt_number}
            </span>
          ) : (
            <span className="flex items-center gap-1">
              <span className="size-2 rounded-full bg-destructive inline-block" />
              Deneme #{r.attempt_number}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

// ── Follow-up prompt preview ──────────────────────────────────────────────

function FollowUpPrompt({ prompt }: { prompt: string }) {
  const [copied, setCopied] = useState(false);
  const copy = useCallback(async () => {
    await navigator.clipboard.writeText(prompt);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [prompt]);

  return (
    <div className="rounded-lg border border-brand/30 bg-brand/5 p-3 space-y-2">
      <div className="flex items-center justify-between">
        <p className="text-xs font-medium text-brand">
          Agent'a Gönderilecek Prompt
        </p>
        <button
          type="button"
          onClick={copy}
          className="flex items-center gap-1 text-xs text-low hover:text-normal transition-colors"
        >
          {copied ? (
            <CheckIcon className="size-3 text-emerald-500" />
          ) : (
            <CopyIcon className="size-3" />
          )}
          {copied ? 'Kopyalandı' : 'Kopyala'}
        </button>
      </div>
      <pre className="text-xs text-low whitespace-pre-wrap font-mono leading-5 max-h-24 overflow-y-auto">
        {prompt}
      </pre>
    </div>
  );
}

// ── Main modal ────────────────────────────────────────────────────────────

interface QaResultModalProps {
  workspaceId: string;
  taskTitle: string;
  onClose: () => void;
  onAction: () => void; // called after force-retry or resolve so parent can refresh
}

export function QaResultModal({
  workspaceId,
  taskTitle,
  onClose,
  onAction,
}: QaResultModalProps) {
  const [detail, setDetail] = useState<OcQaDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeIdx, setActiveIdx] = useState(0);
  const [actionLoading, setActionLoading] = useState(false);

  useEffect(() => {
    openclawApi.getQaDetail(workspaceId).then((d) => {
      setDetail(d);
      if (d) setActiveIdx(d.results.length - 1);
      setLoading(false);
    });
  }, [workspaceId]);

  const handleForceRetry = useCallback(async () => {
    if (!detail) return;
    setActionLoading(true);
    try {
      await openclawApi.forceRetryQa(detail.run.id);
      onAction();
      onClose();
    } finally {
      setActionLoading(false);
    }
  }, [detail, onAction, onClose]);

  const handleResolve = useCallback(async () => {
    if (!detail) return;
    setActionLoading(true);
    try {
      await openclawApi.resolveQa(detail.run.id);
      onAction();
      onClose();
    } finally {
      setActionLoading(false);
    }
  }, [detail, onAction, onClose]);

  const isExhausted = detail?.run.status === 'exhausted';
  const activeResult = detail?.results[activeIdx];

  return (
    /* Overlay */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="w-full max-w-2xl max-h-[85vh] flex flex-col rounded-xl border border-border bg-background shadow-2xl">
        {/* Header */}
        <div className="flex items-center gap-3 px-5 py-4 border-b border-border shrink-0">
          <div className="p-1.5 rounded-md bg-secondary">
            <TerminalIcon className="size-4 text-low" weight="duotone" />
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-semibold truncate">
              QA Sonucu — {taskTitle}
            </p>
            {detail && (
              <p className="text-xs text-low mt-0.5">
                Komut:{' '}
                <code className="font-mono bg-secondary px-1 rounded">
                  {detail.run.test_command}
                </code>
                {'  ·  '}
                {detail.run.retry_count}/{detail.run.max_retries} deneme
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1.5 rounded hover:bg-secondary transition-colors text-low"
          >
            <XIcon className="size-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-5 space-y-4">
          {loading ? (
            <div className="flex items-center gap-2 text-low py-8 justify-center">
              <SpinnerIcon className="size-4 animate-spin" />
              Yükleniyor…
            </div>
          ) : !detail || detail.results.length === 0 ? (
            <p className="text-sm text-low py-8 text-center">
              Bu workspace için henüz QA sonucu yok.
            </p>
          ) : (
            <>
              {/* Exhausted warning banner */}
              {isExhausted && (
                <div className="flex items-start gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3">
                  <span className="text-amber-500 text-lg mt-0.5">⚠️</span>
                  <div>
                    <p className="text-sm font-medium text-amber-500">
                      İnsan Müdahalesi Gerekiyor
                    </p>
                    <p className="text-xs text-low mt-0.5">
                      Agent {detail.run.max_retries} denemede düzeltemedi. Kodu
                      elle inceleyip düzeltebilir ya da tekrar agent'a
                      gönderebilirsin.
                    </p>
                  </div>
                </div>
              )}

              {/* Attempt tabs */}
              {detail.results.length > 1 && (
                <AttemptTabs
                  results={detail.results}
                  activeIdx={activeIdx}
                  onSelect={setActiveIdx}
                />
              )}

              {/* Output */}
              {activeResult && (
                <div className="rounded-lg border border-border bg-[#0d1117] overflow-hidden">
                  {/* Output header */}
                  <div className="flex items-center justify-between px-3 py-2 border-b border-border bg-secondary/50">
                    <span className="text-xs text-low font-mono">
                      Çıktı — exit code:{' '}
                      <span
                        className={
                          activeResult.exit_code === 0
                            ? 'text-emerald-400'
                            : 'text-red-400'
                        }
                      >
                        {activeResult.exit_code ?? '?'}
                      </span>
                    </span>
                  </div>
                  <div className="p-3 max-h-72 overflow-y-auto">
                    {activeResult.output ? (
                      <ColorizedOutput output={activeResult.output} />
                    ) : (
                      <p className="text-xs text-low italic">(çıktı yok)</p>
                    )}
                  </div>
                </div>
              )}

              {/* Follow-up prompt preview */}
              {detail.follow_up_prompt && isExhausted && (
                <FollowUpPrompt prompt={detail.follow_up_prompt} />
              )}
            </>
          )}
        </div>

        {/* Footer actions */}
        {detail && (
          <div className="flex items-center justify-end gap-3 px-5 py-4 border-t border-border shrink-0">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 rounded-md text-sm text-low hover:text-normal hover:bg-secondary transition-colors"
            >
              Kapat
            </button>
            <button
              type="button"
              onClick={handleResolve}
              disabled={actionLoading}
              className={cn(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors',
                'border border-border text-low hover:text-normal hover:bg-secondary'
              )}
            >
              <WrenchIcon className="size-3.5" />
              Elle Düzelt (Geçti Say)
            </button>
            <button
              type="button"
              onClick={handleForceRetry}
              disabled={actionLoading}
              className={cn(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                'bg-brand text-white hover:bg-brand/90',
                actionLoading && 'opacity-50 cursor-not-allowed'
              )}
            >
              {actionLoading ? (
                <SpinnerIcon className="size-3.5 animate-spin" />
              ) : (
                <ArrowCounterClockwiseIcon className="size-3.5" />
              )}
              Agent'a Gönder
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
