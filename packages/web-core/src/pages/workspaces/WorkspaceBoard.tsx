import { useCallback, useMemo, useState } from 'react';
import type { TaskStatus } from 'shared/types';
import {
  KanbanBoard,
  KanbanCard,
  KanbanCards,
  KanbanHeader,
  KanbanProvider,
  type DropResult,
} from '@vibe/ui/components/KanbanBoard';
import {
  IssueWorkspaceCard,
  type WorkspaceWithStats,
} from '@vibe/ui/components/IssueWorkspaceCard';
import {
  useWorkspaces,
  type SidebarWorkspace,
} from '@/shared/hooks/useWorkspaces';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { workspacesApi } from '@/shared/lib/api';

const COLUMNS: { id: TaskStatus; name: string; color: string }[] = [
  { id: 'todo', name: 'To Do', color: '--info' },
  { id: 'inprogress', name: 'In Progress', color: '--brand' },
  { id: 'inreview', name: 'In Review', color: '--warning' },
  { id: 'done', name: 'Done', color: '--success' },
];

function toCardStats(ws: SidebarWorkspace): WorkspaceWithStats {
  return {
    id: ws.id,
    localWorkspaceId: ws.id,
    name: ws.name,
    archived: !!ws.isArchived,
    filesChanged: ws.filesChanged ?? 0,
    linesAdded: ws.linesAdded ?? 0,
    linesRemoved: ws.linesRemoved ?? 0,
    prs:
      ws.prNumber != null &&
      ws.prUrl &&
      ws.prStatus &&
      ws.prStatus !== 'unknown'
        ? [{ number: ws.prNumber, url: ws.prUrl, status: ws.prStatus }]
        : [],
    owner: null,
    updatedAt: ws.updatedAt,
    isOwnedByCurrentUser: true,
    isRunning: ws.isRunning,
    hasPendingApproval: ws.hasPendingApproval,
    hasRunningDevServer: ws.hasRunningDevServer,
    hasUnseenActivity: ws.hasUnseenActivity,
    latestProcessCompletedAt: ws.latestProcessCompletedAt,
    latestProcessStatus: ws.latestProcessStatus,
  };
}

export function WorkspaceBoard() {
  const { workspaces, isLoading } = useWorkspaces();
  const appNavigation = useAppNavigation();
  // Optimistic status overrides while a move is in flight; the WS stream
  // echoes the persisted status back shortly after.
  const [overrides, setOverrides] = useState<Record<string, TaskStatus>>({});

  const columns = useMemo(() => {
    const byStatus: Record<TaskStatus, SidebarWorkspace[]> = {
      todo: [],
      inprogress: [],
      inreview: [],
      done: [],
      cancelled: [],
    };
    for (const ws of workspaces) {
      const status = overrides[ws.id] ?? ws.status;
      (byStatus[status] ?? byStatus.todo).push(ws);
    }
    return byStatus;
  }, [workspaces, overrides]);

  const handleDragEnd = useCallback((result: DropResult) => {
    const { destination, draggableId } = result;
    if (!destination) return;
    const newStatus = destination.droppableId as TaskStatus;
    setOverrides((prev) => ({ ...prev, [draggableId]: newStatus }));
    workspacesApi.update(draggableId, { status: newStatus }).catch(() => {
      // Revert the optimistic move if the server rejected it
      setOverrides((prev) => {
        const next = { ...prev };
        delete next[draggableId];
        return next;
      });
    });
  }, []);

  return (
    <div className="h-full overflow-auto bg-primary">
      <KanbanProvider onDragEnd={handleDragEnd} className="min-w-full">
        {COLUMNS.map((column) => (
          <KanbanBoard key={column.id}>
            <KanbanHeader>
              <div className="sticky top-0 z-20 flex items-center gap-base border-b border-border bg-secondary p-base">
                <div
                  className="h-2 w-2 rounded-full"
                  style={{ backgroundColor: `hsl(var(${column.color}))` }}
                />
                <p className="m-0 text-sm text-high">{column.name}</p>
                <span className="ml-auto text-xs text-low">
                  {columns[column.id].length}
                </span>
              </div>
            </KanbanHeader>
            <KanbanCards id={column.id} className="gap-base p-base">
              {columns[column.id].map((ws, index) => (
                <KanbanCard
                  key={ws.id}
                  id={ws.id}
                  name={ws.name}
                  index={index}
                  className="p-0"
                >
                  <IssueWorkspaceCard
                    workspace={toCardStats(ws)}
                    showOwner={false}
                    onClick={() => appNavigation.goToWorkspace(ws.id)}
                  />
                </KanbanCard>
              ))}
            </KanbanCards>
          </KanbanBoard>
        ))}
      </KanbanProvider>
      {isLoading && <p className="p-base text-sm text-low">Loading…</p>}
    </div>
  );
}
