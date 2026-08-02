import { CaretLeftIcon } from '@phosphor-icons/react';
import { WorkspaceFileTree } from './WorkspaceFileTree';
import { WorkspaceFilesPanel } from './WorkspaceFilesPanel';
import { useWorkspaceFilesStore } from '@/shared/stores/useWorkspaceFilesStore';

interface WorkspaceFilesMobileProps {
  workspaceId: string;
}

/**
 * Mobile flow for the Files feature: full-screen tree, tapping a file
 * opens the full-screen editor with a back button to the tree.
 */
export function WorkspaceFilesMobile({
  workspaceId,
}: WorkspaceFilesMobileProps) {
  const selectedPath = useWorkspaceFilesStore((s) =>
    s.workspaceId === workspaceId ? s.selectedPath : null
  );
  const clearSelection = useWorkspaceFilesStore((s) => s.clearSelection);

  if (!selectedPath) {
    return (
      <div className="h-full overflow-y-auto bg-secondary">
        <WorkspaceFileTree workspaceId={workspaceId} />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-secondary">
      <button
        type="button"
        onClick={clearSelection}
        className="flex shrink-0 items-center gap-1 border-b border-border px-base py-half text-left text-sm text-normal hover:text-high"
      >
        <CaretLeftIcon className="size-icon-xs" weight="bold" />
        All files
      </button>
      <WorkspaceFilesPanel
        workspaceId={workspaceId}
        className="min-h-0 flex-1"
      />
    </div>
  );
}
