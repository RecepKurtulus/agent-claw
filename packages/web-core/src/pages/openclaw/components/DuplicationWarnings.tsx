import { WarningIcon } from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import type { OcDuplicationWarning } from '../oc-types';

interface DuplicationWarningsProps {
  warnings: OcDuplicationWarning[];
}

export function DuplicationWarnings({ warnings }: DuplicationWarningsProps) {
  if (warnings.length === 0) return null;

  return (
    <div className="rounded-md border border-yellow-500/30 bg-yellow-500/5 p-3 space-y-2">
      <div className="flex items-center gap-1.5 text-yellow-500 text-xs font-medium">
        <WarningIcon className="size-icon-xs" weight="fill" />
        <span>
          {warnings.length === 1
            ? 'Benzer bir görev zaten mevcut'
            : `${warnings.length} benzer görev zaten mevcut`}
        </span>
      </div>
      <ul className="space-y-1.5">
        {warnings.map((w, i) => (
          <li key={i} className="text-xs text-low">
            <span className="font-medium text-normal">
              &ldquo;{w.new_task_title}&rdquo;
            </span>{' '}
            →{' '}
            <span className="font-medium text-yellow-400/80">
              &ldquo;{w.similar_task_title}&rdquo;
            </span>{' '}
            <span
              className={cn(
                'ml-1 inline-block px-1 py-0.5 rounded text-[10px] font-medium',
                'bg-secondary text-muted-foreground'
              )}
            >
              {w.existing_status}
            </span>
            <span className="ml-1 text-muted-foreground">
              ({Math.round(w.similarity_score * 100)}% benzer)
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
