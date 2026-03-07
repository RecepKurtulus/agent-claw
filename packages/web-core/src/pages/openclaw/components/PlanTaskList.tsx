import { useState } from 'react';
import {
  TrashIcon,
  PlusIcon,
  DotsSixVerticalIcon,
  ArrowRightIcon,
} from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import type { OcPlanTask, OcTaskComplexity } from '../oc-types';

interface PlanTaskListProps {
  tasks: OcPlanTask[];
  onChange: (tasks: OcPlanTask[]) => void;
}

const COMPLEXITY_STYLES: Record<
  OcTaskComplexity,
  { label: string; className: string }
> = {
  low: { label: 'Düşük', className: 'bg-green-500/15 text-green-400' },
  medium: { label: 'Orta', className: 'bg-yellow-500/15 text-yellow-400' },
  high: { label: 'Yüksek', className: 'bg-red-500/15 text-red-400' },
};

export function PlanTaskList({ tasks, onChange }: PlanTaskListProps) {
  const [editingId, setEditingId] = useState<string | null>(null);

  function deleteTask(id: string) {
    onChange(tasks.filter((t) => t.id !== id));
  }

  function updateTask(id: string, patch: Partial<OcPlanTask>) {
    onChange(tasks.map((t) => (t.id === id ? { ...t, ...patch } : t)));
  }

  function addTask() {
    const newTask: OcPlanTask = {
      id: crypto.randomUUID(),
      plan_id: tasks[0]?.plan_id ?? '',
      title: '',
      description: '',
      estimated_complexity: 'medium',
      depends_on: [],
      order_index: tasks.length,
      created_at: new Date().toISOString(),
    };
    onChange([...tasks, newTask]);
    setEditingId(newTask.id);
  }

  return (
    <div className="space-y-2">
      {tasks.length === 0 && (
        <p className="text-xs text-muted-foreground text-center py-4">
          Henüz görev yok. Aşağıdan ekleyebilirsiniz.
        </p>
      )}

      {tasks.map((task, index) => (
        <TaskRow
          key={task.id}
          task={task}
          index={index}
          isEditing={editingId === task.id}
          onEdit={() => setEditingId(task.id)}
          onBlur={() => setEditingId(null)}
          onUpdate={(patch) => updateTask(task.id, patch)}
          onDelete={() => deleteTask(task.id)}
        />
      ))}

      <button
        type="button"
        onClick={addTask}
        className={cn(
          'w-full flex items-center justify-center gap-1.5',
          'py-2 rounded-md border border-dashed border-border',
          'text-xs text-low hover:text-normal hover:border-brand/40',
          'transition-colors'
        )}
      >
        <PlusIcon className="size-icon-xs" weight="bold" />
        Görev Ekle
      </button>
    </div>
  );
}

// ── TaskRow ────────────────────────────────────────────────────────────────

interface TaskRowProps {
  task: OcPlanTask;
  index: number;
  isEditing: boolean;
  onEdit: () => void;
  onBlur: () => void;
  onUpdate: (patch: Partial<OcPlanTask>) => void;
  onDelete: () => void;
}

function TaskRow({
  task,
  index,
  isEditing,
  onEdit,
  onBlur,
  onUpdate,
  onDelete,
}: TaskRowProps) {
  const complexity = COMPLEXITY_STYLES[task.estimated_complexity];

  return (
    <div
      className={cn(
        'group flex items-start gap-2 p-2.5 rounded-md',
        'border border-border bg-panel',
        'hover:border-brand/20 transition-colors'
      )}
    >
      {/* Drag handle + index */}
      <div className="flex items-center gap-1 pt-0.5 text-low opacity-0 group-hover:opacity-100 transition-opacity">
        <DotsSixVerticalIcon className="size-icon-xs" />
        <span className="text-[10px] font-mono w-4 text-center">
          {index + 1}
        </span>
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0 space-y-1">
        {/* Title row */}
        <div className="flex items-start gap-2">
          {isEditing ? (
            <input
              autoFocus
              value={task.title}
              onChange={(e) => onUpdate({ title: e.target.value })}
              onBlur={onBlur}
              className={cn(
                'flex-1 bg-transparent text-sm font-medium text-normal',
                'border-b border-brand outline-none pb-0.5'
              )}
              placeholder="Görev başlığı..."
            />
          ) : (
            <button
              type="button"
              onClick={onEdit}
              className={cn(
                'flex-1 text-left text-sm font-medium text-normal truncate',
                'hover:text-brand transition-colors',
                !task.title && 'text-muted-foreground italic'
              )}
            >
              {task.title || 'Başlık ekle...'}
            </button>
          )}

          {/* Complexity badge */}
          <span
            className={cn(
              'shrink-0 text-[10px] font-medium px-1.5 py-0.5 rounded-full',
              complexity.className
            )}
          >
            {complexity.label}
          </span>
        </div>

        {/* Description */}
        {(task.description || isEditing) && (
          <p
            className={cn(
              'text-xs text-low line-clamp-2 cursor-text',
              isEditing && 'hidden'
            )}
            onClick={onEdit}
          >
            {task.description || (
              <span className="italic text-muted-foreground">
                Açıklama ekle...
              </span>
            )}
          </p>
        )}

        {/* Depends on */}
        {task.depends_on.length > 0 && (
          <div className="flex items-center gap-1 flex-wrap">
            <ArrowRightIcon className="size-[10px] text-low" weight="bold" />
            {task.depends_on.map((dep) => (
              <span
                key={dep}
                className={cn(
                  'text-[10px] px-1.5 py-0.5 rounded-full',
                  'bg-secondary text-muted-foreground'
                )}
              >
                {dep}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Delete button */}
      <button
        type="button"
        onClick={onDelete}
        className={cn(
          'shrink-0 p-1 rounded-sm mt-0.5',
          'text-low opacity-0 group-hover:opacity-100',
          'hover:text-red-400 hover:bg-red-500/10',
          'transition-all'
        )}
        aria-label="Görevi sil"
      >
        <TrashIcon className="size-icon-xs" />
      </button>
    </div>
  );
}
