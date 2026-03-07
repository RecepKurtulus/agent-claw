import { useState, useCallback } from 'react';
import {
  BrainIcon,
  SpinnerIcon,
  WarningCircleIcon,
  ListBulletsIcon,
  GraphIcon,
} from '@phosphor-icons/react';
import { create, useModal } from '@ebay/nice-modal-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@vibe/ui/components/Dialog';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { cn } from '@/shared/lib/utils';
import { defineModal } from '@/shared/lib/modals';
import { useCreateOcPlan } from './hooks/useCreateOcPlan';
import { DuplicationWarnings } from './components/DuplicationWarnings';
import { PlanTaskList } from './components/PlanTaskList';
import { DependencyGraph } from './components/DependencyGraph';
import type { OcPlanTask, CreateOcPlanResponse } from './oc-types';

// ── Dialog props ───────────────────────────────────────────────────────────

export interface OpenClawDialogProps {
  projectId: string;
  repoPaths?: string[];
}

// ── Step types ─────────────────────────────────────────────────────────────

type Step = 'idle' | 'loading' | 'result' | 'error';

// ── Dialog implementation ──────────────────────────────────────────────────

function OpenClawDialogImpl({ projectId, repoPaths }: OpenClawDialogProps) {
  const modal = useModal();
  const createPlan = useCreateOcPlan();

  const [step, setStep] = useState<Step>('idle');
  const [prompt, setPrompt] = useState('');
  const [result, setResult] = useState<CreateOcPlanResponse | null>(null);
  const [draftTasks, setDraftTasks] = useState<OcPlanTask[]>([]);
  const [errorMessage, setErrorMessage] = useState('');

  const handleAnalyze = useCallback(async () => {
    if (!prompt.trim()) return;
    setStep('loading');
    try {
      const res = await createPlan.mutateAsync({
        project_id: projectId,
        prompt: prompt.trim(),
        repo_paths: repoPaths,
      });
      setResult(res);
      setDraftTasks(res.tasks);
      setStep('result');
    } catch (err) {
      setErrorMessage(
        err instanceof Error ? err.message : 'Bilinmeyen bir hata oluştu.'
      );
      setStep('error');
    }
  }, [prompt, projectId, repoPaths, createPlan]);

  const handleConfirm = useCallback(() => {
    modal.resolve(draftTasks);
    modal.hide();
  }, [modal, draftTasks]);

  const handleBack = useCallback(() => {
    setStep('idle');
    setResult(null);
    setErrorMessage('');
  }, []);

  return (
    <Dialog open={modal.visible} onOpenChange={modal.hide}>
      <DialogContent className="max-w-2xl max-h-[85vh] flex flex-col gap-0 p-0">
        {/* Header */}
        <DialogHeader className="px-5 pt-5 pb-4 border-b border-border shrink-0">
          <div className="flex items-center gap-2">
            <div className="p-1.5 rounded-md bg-brand/10">
              <BrainIcon className="size-icon-sm text-brand" weight="duotone" />
            </div>
            <div>
              <DialogTitle className="text-base font-semibold">
                OpenClaw Planlayıcı
              </DialogTitle>
              <DialogDescription className="text-xs text-low mt-0.5">
                Tek cümleyle söyle, görev listesi otomatik oluşsun.
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-4">
          {/* ── Step: idle ── */}
          {step === 'idle' && (
            <PromptStep
              prompt={prompt}
              onChange={setPrompt}
              onSubmit={handleAnalyze}
            />
          )}

          {/* ── Step: loading ── */}
          {step === 'loading' && <LoadingStep />}

          {/* ── Step: result ── */}
          {step === 'result' && result && (
            <ResultStep
              result={result}
              draftTasks={draftTasks}
              onTasksChange={setDraftTasks}
            />
          )}

          {/* ── Step: error ── */}
          {step === 'error' && (
            <ErrorStep message={errorMessage} onRetry={handleBack} />
          )}
        </div>

        {/* Footer */}
        <div
          className={cn(
            'px-5 py-4 border-t border-border shrink-0',
            'flex items-center justify-between gap-3'
          )}
        >
          {step === 'result' && (
            <p className="text-xs text-low">{draftTasks.length} görev hazır</p>
          )}
          <div className="flex items-center gap-2 ml-auto">
            <button
              type="button"
              onClick={step === 'result' ? handleBack : modal.hide}
              className="px-3 py-1.5 rounded-md text-sm text-low hover:text-normal hover:bg-secondary transition-colors"
            >
              {step === 'result' ? 'Geri' : 'İptal'}
            </button>

            {step === 'idle' && (
              <PrimaryButton onClick={handleAnalyze} disabled={!prompt.trim()}>
                Analiz Et
              </PrimaryButton>
            )}

            {step === 'result' && (
              <PrimaryButton
                onClick={handleConfirm}
                disabled={draftTasks.length === 0}
              >
                Planı Onayla ({draftTasks.length})
              </PrimaryButton>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ── Step components ────────────────────────────────────────────────────────

function PromptStep({
  prompt,
  onChange,
  onSubmit,
}: {
  prompt: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
}) {
  return (
    <div className="space-y-3">
      <label className="block text-sm font-medium text-normal">
        Ne yapmak istiyorsunuz?
      </label>
      <textarea
        autoFocus
        value={prompt}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
            onSubmit();
          }
        }}
        rows={5}
        placeholder={
          'Örnek: "Kullanıcı kayıt ve giriş sistemi ekle. JWT token ile auth, e-posta doğrulama ve şifre sıfırlama gerekli."'
        }
        className={cn(
          'w-full rounded-md border border-border bg-panel',
          'px-3 py-2.5 text-sm text-normal placeholder:text-muted-foreground',
          'focus:outline-none focus:border-brand/50 focus:ring-1 focus:ring-brand/20',
          'resize-none transition-colors'
        )}
      />
      <p className="text-xs text-low">
        ⌘ + Enter ile de analiz başlatabilirsiniz.
      </p>
    </div>
  );
}

function LoadingStep() {
  return (
    <div className="flex flex-col items-center justify-center py-12 space-y-3">
      <SpinnerIcon className="size-8 text-brand animate-spin" weight="bold" />
      <p className="text-sm text-normal font-medium">Analiz ediliyor...</p>
      <p className="text-xs text-low text-center max-w-xs">
        Kod tabanı taranıyor, bağımlılıklar hesaplanıyor ve görevler
        oluşturuluyor.
      </p>
    </div>
  );
}

function ResultStep({
  result,
  draftTasks,
  onTasksChange,
}: {
  result: CreateOcPlanResponse;
  draftTasks: OcPlanTask[];
  onTasksChange: (tasks: OcPlanTask[]) => void;
}) {
  const [view, setView] = useState<'list' | 'graph'>('list');

  return (
    <div className="space-y-4">
      {/* Codebase context badge */}
      {result.codebase_context && (
        <div className="flex items-center gap-2 text-xs text-low bg-secondary rounded-md px-3 py-2">
          <BrainIcon className="size-icon-xs text-brand" weight="duotone" />
          <span>
            <span className="font-medium text-normal">
              {result.codebase_context.project_type}
            </span>{' '}
            projesi tespit edildi — {result.codebase_context.key_file_count}{' '}
            anahtar dosya, {result.codebase_context.existing_task_count} mevcut
            görev incelendi.
          </span>
        </div>
      )}

      {/* Duplication warnings */}
      <DuplicationWarnings warnings={result.duplication_warnings} />

      {/* View toggle + task list/graph */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium text-normal">
            Oluşturulan Görevler
          </h3>
          <div className="flex items-center gap-1 bg-secondary rounded-md p-0.5 border border-border">
            <button
              type="button"
              onClick={() => setView('list')}
              className={cn(
                'flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors',
                view === 'list'
                  ? 'bg-panel text-normal shadow-sm'
                  : 'text-low hover:text-normal'
              )}
            >
              <ListBulletsIcon className="size-3" />
              Liste
            </button>
            <button
              type="button"
              onClick={() => setView('graph')}
              className={cn(
                'flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors',
                view === 'graph'
                  ? 'bg-panel text-normal shadow-sm'
                  : 'text-low hover:text-normal'
              )}
            >
              <GraphIcon className="size-3" />
              Graf
            </button>
          </div>
        </div>

        {view === 'list' && (
          <PlanTaskList tasks={draftTasks} onChange={onTasksChange} />
        )}
        {view === 'graph' && (
          <DependencyGraph tasks={draftTasks} onChange={onTasksChange} />
        )}
      </div>
    </div>
  );
}

function ErrorStep({
  message,
  onRetry,
}: {
  message: string;
  onRetry: () => void;
}) {
  return (
    <div className="flex flex-col items-center justify-center py-10 space-y-3">
      <WarningCircleIcon className="size-8 text-red-400" weight="duotone" />
      <p className="text-sm font-medium text-normal">Analiz başarısız</p>
      <p className="text-xs text-low text-center max-w-sm">{message}</p>
      <button
        type="button"
        onClick={onRetry}
        className="mt-2 px-3 py-1.5 rounded-md text-sm text-brand hover:bg-brand/10 transition-colors"
      >
        Tekrar Dene
      </button>
    </div>
  );
}

// ── Export as NiceModal ────────────────────────────────────────────────────

export const OpenClawDialog = defineModal<OpenClawDialogProps, OcPlanTask[]>(
  create(OpenClawDialogImpl)
);
