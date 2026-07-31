-- Add a kanban status to workspaces (same value set as the tasks table)
ALTER TABLE workspaces
    ADD COLUMN status TEXT NOT NULL DEFAULT 'todo'
    CHECK (status IN ('todo', 'inprogress', 'done', 'cancelled', 'inreview'));
