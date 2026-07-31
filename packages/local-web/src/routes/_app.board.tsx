import { createFileRoute } from '@tanstack/react-router';
import { WorkspaceBoard } from '@/pages/workspaces/WorkspaceBoard';

export const Route = createFileRoute('/_app/board')({
  component: WorkspaceBoard,
});
