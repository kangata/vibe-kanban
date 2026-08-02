import { useCallback, useEffect, useState } from 'react';
import {
  CaretDownIcon,
  CaretRightIcon,
  FileIcon,
  FolderIcon,
  FolderOpenIcon,
} from '@phosphor-icons/react';
import type { WorkspaceFileEntry } from 'shared/types';
import { workspaceFilesApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import { useWorkspaceFilesStore } from '@/shared/stores/useWorkspaceFilesStore';

interface WorkspaceFileTreeProps {
  workspaceId: string;
}

interface TreeNodeProps {
  entry: WorkspaceFileEntry;
  workspaceId: string;
  depth: number;
}

function TreeNode({ entry, workspaceId, depth }: TreeNodeProps) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<WorkspaceFileEntry[] | null>(null);
  const selectedPath = useWorkspaceFilesStore((s) =>
    s.workspaceId === workspaceId ? s.selectedPath : null
  );
  const selectFile = useWorkspaceFilesStore((s) => s.selectFile);

  const handleClick = useCallback(() => {
    if (entry.is_dir) {
      const next = !expanded;
      setExpanded(next);
      if (next && children === null) {
        workspaceFilesApi
          .list(workspaceId, entry.path)
          .then(setChildren)
          .catch(() => setChildren([]));
      }
    } else {
      selectFile(workspaceId, entry.path);
    }
  }, [entry, expanded, children, workspaceId, selectFile]);

  const Chevron = expanded ? CaretDownIcon : CaretRightIcon;
  const DirIcon = expanded ? FolderOpenIcon : FolderIcon;

  return (
    <>
      <button
        type="button"
        onClick={handleClick}
        className={cn(
          'flex w-full items-center gap-1 rounded-xs px-1 py-0.5 text-left text-sm',
          selectedPath === entry.path
            ? 'bg-brand/15 text-high'
            : 'text-normal hover:bg-primary'
        )}
        style={{ paddingLeft: `${depth * 12 + 4}px` }}
        title={entry.path}
      >
        {entry.is_dir ? (
          <>
            <Chevron className="size-icon-xs shrink-0 text-low" />
            <DirIcon className="size-icon-xs shrink-0 text-low" />
          </>
        ) : (
          <FileIcon className="ml-[14px] size-icon-xs shrink-0 text-low" />
        )}
        <span className="truncate">{entry.name}</span>
      </button>
      {entry.is_dir &&
        expanded &&
        children?.map((child) => (
          <TreeNode
            key={child.path}
            entry={child}
            workspaceId={workspaceId}
            depth={depth + 1}
          />
        ))}
    </>
  );
}

export function WorkspaceFileTree({ workspaceId }: WorkspaceFileTreeProps) {
  const [rootEntries, setRootEntries] = useState<WorkspaceFileEntry[] | null>(
    null
  );
  const [rootError, setRootError] = useState<string | null>(null);

  useEffect(() => {
    setRootEntries(null);
    setRootError(null);
    workspaceFilesApi
      .list(workspaceId, '')
      .then(setRootEntries)
      .catch((e) => setRootError(e?.message ?? 'Failed to load files'));
  }, [workspaceId]);

  return (
    <div className="w-full overflow-y-auto p-half">
      {rootError && <p className="p-base text-sm text-low">{rootError}</p>}
      {!rootEntries && !rootError && (
        <p className="p-base text-sm text-low">Loading…</p>
      )}
      {rootEntries?.map((entry) => (
        <TreeNode
          key={entry.path}
          entry={entry}
          workspaceId={workspaceId}
          depth={0}
        />
      ))}
    </div>
  );
}
