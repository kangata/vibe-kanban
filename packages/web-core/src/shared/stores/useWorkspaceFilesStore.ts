import { create } from 'zustand';

interface WorkspaceFilesState {
  /** Workspace the selected file belongs to. */
  workspaceId: string | null;
  selectedPath: string | null;
  selectFile: (workspaceId: string, path: string) => void;
}

/** Shares the file selected in the sidebar tree with the main editor panel. */
export const useWorkspaceFilesStore = create<WorkspaceFilesState>((set) => ({
  workspaceId: null,
  selectedPath: null,
  selectFile: (workspaceId, selectedPath) => set({ workspaceId, selectedPath }),
}));
