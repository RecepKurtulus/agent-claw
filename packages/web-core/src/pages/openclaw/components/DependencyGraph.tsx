import { useCallback, useMemo } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  addEdge,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  type OnConnect,
  type NodeProps,
  Handle,
  Position,
  MarkerType,
  Panel,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { XIcon } from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import type { OcPlanTask } from '../oc-types';

// ── Cycle detection ────────────────────────────────────────────────────────

function detectCycles(edges: Edge[]): Set<string> {
  const adj = new Map<string, string[]>();
  for (const e of edges) {
    if (!adj.has(e.source)) adj.set(e.source, []);
    adj.get(e.source)!.push(e.target);
  }

  const cycleEdges = new Set<string>();
  const visited = new Set<string>();
  const inStack = new Set<string>();

  function dfs(node: string, path: string[]): boolean {
    visited.add(node);
    inStack.add(node);
    path.push(node);

    for (const neighbor of adj.get(node) ?? []) {
      if (!visited.has(neighbor)) {
        if (dfs(neighbor, path)) return true;
      } else if (inStack.has(neighbor)) {
        // Found cycle — mark the edge
        cycleEdges.add(`${node}->${neighbor}`);
        return true;
      }
    }
    inStack.delete(node);
    path.pop();
    return false;
  }

  for (const node of adj.keys()) {
    if (!visited.has(node)) dfs(node, []);
  }

  return cycleEdges;
}

// ── Custom node ────────────────────────────────────────────────────────────

type TaskNodeData = {
  label: string;
  complexity: string;
  hasCycle: boolean;
  onDelete: (id: string) => void;
};

function TaskNode({ id, data }: NodeProps<Node<TaskNodeData>>) {
  const complexityColor =
    data.complexity === 'high'
      ? 'text-red-400'
      : data.complexity === 'medium'
        ? 'text-yellow-400'
        : 'text-green-400';

  return (
    <div
      className={cn(
        'relative bg-secondary border rounded-md px-3 py-2 min-w-[140px] max-w-[200px] shadow-sm',
        data.hasCycle ? 'border-red-400 shadow-red-400/20' : 'border-border'
      )}
    >
      <Handle
        type="target"
        position={Position.Top}
        className="!bg-brand !border-brand/50 !w-2 !h-2"
      />

      <div className="flex items-start justify-between gap-1">
        <p className="text-xs font-medium text-normal leading-snug line-clamp-3 flex-1">
          {data.label}
        </p>
        <button
          type="button"
          onClick={() => data.onDelete(id)}
          className="shrink-0 p-0.5 rounded text-muted-foreground hover:text-red-400 transition-colors mt-0.5"
          title="Görevi sil"
        >
          <XIcon className="size-3" />
        </button>
      </div>

      <span
        className={cn('text-[10px] font-medium mt-1 block', complexityColor)}
      >
        {data.complexity}
      </span>

      {data.hasCycle && (
        <div className="mt-1 text-[9px] text-red-400 font-medium">
          ⚠ Döngü!
        </div>
      )}

      <Handle
        type="source"
        position={Position.Bottom}
        className="!bg-brand !border-brand/50 !w-2 !h-2"
      />
    </div>
  );
}

const nodeTypes = { task: TaskNode };

// ── Auto layout ────────────────────────────────────────────────────────────

function buildLayout(tasks: OcPlanTask[]): { x: number; y: number }[] {
  const LAYER_HEIGHT = 110;
  const NODE_WIDTH = 200;
  const NODE_GAP = 20;

  // Group by order_index
  const layers = new Map<number, number[]>();
  tasks.forEach((t, i) => {
    const layer = t.order_index ?? 0;
    if (!layers.has(layer)) layers.set(layer, []);
    layers.get(layer)!.push(i);
  });

  const positions: { x: number; y: number }[] = new Array(tasks.length);
  const sortedLayers = [...layers.entries()].sort((a, b) => a[0] - b[0]);

  sortedLayers.forEach(([_layerIdx, nodeIdxs], layerPos) => {
    const totalWidth =
      nodeIdxs.length * NODE_WIDTH + (nodeIdxs.length - 1) * NODE_GAP;
    const startX = -totalWidth / 2;
    nodeIdxs.forEach((nodeIdx, col) => {
      positions[nodeIdx] = {
        x: startX + col * (NODE_WIDTH + NODE_GAP),
        y: layerPos * LAYER_HEIGHT,
      };
    });
  });

  return positions;
}

// ── Main component ─────────────────────────────────────────────────────────

interface DependencyGraphProps {
  tasks: OcPlanTask[];
  onChange: (tasks: OcPlanTask[]) => void;
}

export function DependencyGraph({ tasks, onChange }: DependencyGraphProps) {
  const positions = useMemo(() => buildLayout(tasks), [tasks]);

  const handleDeleteNode = useCallback(
    (id: string) => {
      onChange(
        tasks
          .filter((t) => t.id !== id)
          .map((t) => ({
            ...t,
            depends_on: t.depends_on.filter((dep) => dep !== id),
          }))
      );
    },
    [tasks, onChange]
  );

  const initialNodes: Node<TaskNodeData>[] = useMemo(
    () =>
      tasks.map((t, i) => ({
        id: t.id,
        type: 'task',
        position: positions[i] ?? { x: i * 220, y: 0 },
        data: {
          label: t.title,
          complexity: t.estimated_complexity,
          hasCycle: false,
          onDelete: handleDeleteNode,
        },
      })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [tasks.map((t) => t.id).join(','), handleDeleteNode]
  );

  const initialEdges: Edge[] = useMemo(
    () =>
      tasks.flatMap((t) =>
        t.depends_on.map((depId) => ({
          id: `${depId}->${t.id}`,
          source: depId,
          target: t.id,
          markerEnd: {
            type: MarkerType.ArrowClosed,
            color: 'var(--color-brand)',
          },
          style: { stroke: 'var(--color-brand)', strokeWidth: 1.5 },
          animated: false,
        }))
      ),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [tasks.map((t) => t.id + t.depends_on.join('')).join(',')]
  );

  const [nodes, , onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  // Highlight cycles
  const cycleEdgeKeys = useMemo(() => detectCycles(edges), [edges]);

  const styledEdges = useMemo(
    () =>
      edges.map((e) =>
        cycleEdgeKeys.has(`${e.source}->${e.target}`)
          ? {
              ...e,
              style: { stroke: '#f87171', strokeWidth: 2 },
              markerEnd: { type: MarkerType.ArrowClosed, color: '#f87171' },
              animated: true,
            }
          : e
      ),
    [edges, cycleEdgeKeys]
  );

  const cycleNodeIds = useMemo(() => {
    const ids = new Set<string>();
    for (const key of cycleEdgeKeys) {
      const [src, tgt] = key.split('->');
      ids.add(src);
      ids.add(tgt);
    }
    return ids;
  }, [cycleEdgeKeys]);

  const styledNodes = useMemo(
    () =>
      nodes.map((n) => ({
        ...n,
        data: { ...n.data, hasCycle: cycleNodeIds.has(n.id) },
      })),
    [nodes, cycleNodeIds]
  );

  const onConnect: OnConnect = useCallback(
    (connection) => {
      setEdges((eds) =>
        addEdge(
          {
            ...connection,
            markerEnd: {
              type: MarkerType.ArrowClosed,
              color: 'var(--color-brand)',
            },
            style: { stroke: 'var(--color-brand)', strokeWidth: 1.5 },
          },
          eds
        )
      );
      // Update tasks depends_on
      onChange(
        tasks.map((t) =>
          t.id === connection.target
            ? {
                ...t,
                depends_on: [...new Set([...t.depends_on, connection.source!])],
              }
            : t
        )
      );
    },
    [setEdges, tasks, onChange]
  );

  const onEdgeClick = useCallback(
    (_: React.MouseEvent, edge: Edge) => {
      setEdges((eds) => eds.filter((e) => e.id !== edge.id));
      onChange(
        tasks.map((t) =>
          t.id === edge.target
            ? {
                ...t,
                depends_on: t.depends_on.filter((d) => d !== edge.source),
              }
            : t
        )
      );
    },
    [setEdges, tasks, onChange]
  );

  const hasCycle = cycleEdgeKeys.size > 0;

  return (
    <div
      className="relative rounded-md border border-border overflow-hidden bg-panel"
      style={{ height: 360 }}
    >
      <ReactFlow
        nodes={styledNodes}
        edges={styledEdges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onEdgeClick={onEdgeClick}
        nodeTypes={nodeTypes}
        fitView
        fitViewOptions={{ padding: 0.3 }}
        deleteKeyCode={null}
        proOptions={{ hideAttribution: true }}
      >
        <Background gap={16} size={1} className="opacity-30" />
        <Controls
          showInteractive={false}
          className="!bg-secondary !border-border !shadow-none"
        />
        {hasCycle && (
          <Panel position="top-center">
            <div className="bg-red-950/90 text-red-300 text-xs px-3 py-1 rounded-full border border-red-800">
              ⚠ Döngüsel bağımlılık tespit edildi — kırmızı okları kaldırın
            </div>
          </Panel>
        )}
        <Panel position="bottom-right">
          <div className="text-[10px] text-low bg-secondary/80 px-2 py-1 rounded border border-border">
            Alt handle'dan sürükle → bağımlılık ekle · Oka tıkla → sil
          </div>
        </Panel>
      </ReactFlow>
    </div>
  );
}
