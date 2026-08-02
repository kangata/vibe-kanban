import { useCallback, useEffect, useMemo, useState } from 'react';
import CodeMirror, { type Extension } from '@uiw/react-codemirror';
import { langs } from '@uiw/codemirror-extensions-langs';
import {
  CaretDownIcon,
  CaretRightIcon,
  FileIcon,
  FloppyDiskIcon,
  FolderIcon,
  FolderOpenIcon,
  SpinnerIcon,
} from '@phosphor-icons/react';
import type { WorkspaceFileEntry } from 'shared/types';
import { useTheme } from '@/shared/hooks/useTheme';
import { getEditorTheme } from '@/shared/lib/editorTheme';
import { workspaceFilesApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';

interface WorkspaceFilesPanelProps {
  workspaceId: string;
  className?: string;
}

const EXTENSION_LANGUAGES: Record<string, () => Extension> = {
  ts: langs.ts,
  tsx: langs.tsx,
  js: langs.js,
  jsx: langs.jsx,
  json: langs.json,
  rs: langs.rs,
  py: langs.py,
  php: langs.php,
  html: langs.html,
  css: langs.css,
  scss: langs.scss,
  md: langs.markdown,
  yml: langs.yml,
  yaml: langs.yaml,
  sql: langs.sql,
  sh: langs.sh,
  vue: langs.vue,
  go: langs.go,
  java: langs.java,
  xml: langs.xml,
  toml: langs.toml,
};

function languageExtensionsFor(path: string): Extension[] {
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  const language = EXTENSION_LANGUAGES[ext];
  return language ? [language()] : [];
}

interface TreeNodeProps {
  entry: WorkspaceFileEntry;
  workspaceId: string;
  depth: number;
  selectedPath: string | null;
  onSelectFile: (path: string) => void;
}

function TreeNode({
  entry,
  workspaceId,
  depth,
  selectedPath,
  onSelectFile,
}: TreeNodeProps) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<WorkspaceFileEntry[] | null>(null);

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
      onSelectFile(entry.path);
    }
  }, [entry, expanded, children, workspaceId, onSelectFile]);

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
            selectedPath={selectedPath}
            onSelectFile={onSelectFile}
          />
        ))}
    </>
  );
}

export function WorkspaceFilesPanel({
  workspaceId,
  className,
}: WorkspaceFilesPanelProps) {
  const { theme } = useTheme();
  const [rootEntries, setRootEntries] = useState<WorkspaceFileEntry[] | null>(
    null
  );
  const [rootError, setRootError] = useState<string | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [content, setContent] = useState<string>('');
  const [savedContent, setSavedContent] = useState<string>('');
  const [fileError, setFileError] = useState<string | null>(null);
  const [loadingFile, setLoadingFile] = useState(false);
  const [saving, setSaving] = useState(false);

  const isDirty = content !== savedContent;
  // Rebuilt when the app theme changes: reads current CSS variables so the
  // editor colors always match the surrounding UI.
  const editorTheme = useMemo(() => getEditorTheme(), [theme]);

  useEffect(() => {
    setRootEntries(null);
    setRootError(null);
    setSelectedPath(null);
    setContent('');
    setSavedContent('');
    workspaceFilesApi
      .list(workspaceId, '')
      .then(setRootEntries)
      .catch((e) => setRootError(e?.message ?? 'Failed to load files'));
  }, [workspaceId]);

  const handleSelectFile = useCallback(
    (path: string) => {
      setSelectedPath(path);
      setFileError(null);
      setLoadingFile(true);
      workspaceFilesApi
        .read(workspaceId, path)
        .then((file) => {
          setContent(file.content);
          setSavedContent(file.content);
        })
        .catch((e) => {
          setContent('');
          setSavedContent('');
          setFileError(e?.message ?? 'Failed to read file');
        })
        .finally(() => setLoadingFile(false));
    },
    [workspaceId]
  );

  const handleSave = useCallback(() => {
    if (!selectedPath || !isDirty || saving) return;
    setSaving(true);
    workspaceFilesApi
      .write(workspaceId, selectedPath, content)
      .then((file) => setSavedContent(file.content))
      .catch((e) => setFileError(e?.message ?? 'Failed to save file'))
      .finally(() => setSaving(false));
  }, [workspaceId, selectedPath, content, isDirty, saving]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault();
        handleSave();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [handleSave]);

  const languageExtensions = useMemo(
    () => (selectedPath ? languageExtensionsFor(selectedPath) : []),
    [selectedPath]
  );

  return (
    <div className={cn('flex h-full min-h-0 bg-secondary', className)}>
      {/* File tree */}
      <div className="w-[240px] shrink-0 overflow-y-auto border-r border-border p-half">
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
            selectedPath={selectedPath}
            onSelectFile={handleSelectFile}
          />
        ))}
      </div>

      {/* Editor */}
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex h-9 shrink-0 items-center gap-base border-b border-border px-base">
          <span className="truncate text-sm text-normal">
            {selectedPath ?? 'Select a file'}
            {isDirty && <span className="ml-1 text-brand">●</span>}
          </span>
          <div className="ml-auto flex items-center gap-half">
            {saving && (
              <SpinnerIcon className="size-icon-xs animate-spin text-low" />
            )}
            <button
              type="button"
              onClick={handleSave}
              disabled={!isDirty || saving}
              className={cn(
                'flex items-center gap-1 rounded-sm px-2 py-0.5 text-xs',
                isDirty
                  ? 'bg-brand text-on-brand hover:bg-brand-hover cursor-pointer'
                  : 'bg-panel text-low cursor-default'
              )}
            >
              <FloppyDiskIcon className="size-icon-xs" />
              Save
            </button>
          </div>
        </div>
        {fileError && (
          <p className="border-b border-border p-base text-sm text-low">
            {fileError}
          </p>
        )}
        <div className="min-h-0 flex-1 overflow-auto">
          {loadingFile ? (
            <p className="p-base text-sm text-low">Loading…</p>
          ) : selectedPath && !fileError ? (
            <CodeMirror
              value={content}
              height="100%"
              theme={editorTheme}
              extensions={languageExtensions}
              onChange={setContent}
              style={{ height: '100%' }}
            />
          ) : (
            <div className="flex h-full items-center justify-center">
              <p className="text-sm text-low">
                Select a file from the tree to view or edit it
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
