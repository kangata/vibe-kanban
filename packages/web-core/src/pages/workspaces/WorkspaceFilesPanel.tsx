import { useCallback, useEffect, useMemo, useState } from 'react';
import CodeMirror, { type Extension } from '@uiw/react-codemirror';
import { langs } from '@uiw/codemirror-extensions-langs';
import { FloppyDiskIcon, SpinnerIcon } from '@phosphor-icons/react';
import { useTheme } from '@/shared/hooks/useTheme';
import { getEditorTheme } from '@/shared/lib/editorTheme';
import { workspaceFilesApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import { useWorkspaceFilesStore } from '@/shared/stores/useWorkspaceFilesStore';

interface WorkspaceFilesPanelProps {
  workspaceId: string;
  className?: string;
}

const EXTENSION_LANGUAGES: Record<string, () => Extension> = {
  ts: langs.ts,
  cts: langs.ts,
  mts: langs.ts,
  tsx: langs.tsx,
  js: langs.js,
  cjs: langs.js,
  mjs: langs.js,
  jsx: langs.jsx,
  json: langs.json,
  rs: langs.rs,
  py: langs.py,
  php: langs.php,
  html: langs.html,
  htm: langs.html,
  css: langs.css,
  scss: langs.scss,
  md: langs.markdown,
  yml: langs.yml,
  yaml: langs.yaml,
  sql: langs.sql,
  sh: langs.sh,
  bash: langs.bash,
  zsh: langs.sh,
  vue: langs.vue,
  go: langs.go,
  java: langs.java,
  xml: langs.xml,
  toml: langs.toml,
  env: langs.properties,
  ini: langs.ini,
  conf: langs.ini,
  cfg: langs.ini,
  properties: langs.properties,
  diff: langs.diff,
  patch: langs.diff,
  txt: langs.text,
};

/** Exact (lowercased) filenames that don't follow extension conventions. */
const FILENAME_LANGUAGES: Record<string, () => Extension> = {
  'composer.lock': langs.json,
  'yarn.lock': langs.yaml,
  '.editorconfig': langs.ini,
  '.gitmodules': langs.ini,
  '.gitconfig': langs.ini,
  '.gitignore': langs.properties,
  '.gitattributes': langs.properties,
  '.dockerignore': langs.properties,
  '.npmrc': langs.properties,
  '.nvmrc': langs.text,
  '.prettierrc': langs.json,
  '.babelrc': langs.json,
  '.eslintrc': langs.json,
  dockerfile: langs.sh,
  makefile: langs.sh,
  procfile: langs.yaml,
};

function languageExtensionsFor(path: string): Extension[] {
  const basename = path.split('/').pop()?.toLowerCase() ?? '';

  // .env, .env.example, .env.local, ...
  if (basename.startsWith('.env')) return [langs.properties()];

  const byName = FILENAME_LANGUAGES[basename];
  if (byName) return [byName()];

  const ext = basename.includes('.') ? (basename.split('.').pop() ?? '') : '';
  const byExt = EXTENSION_LANGUAGES[ext];
  return byExt ? [byExt()] : [];
}

export function WorkspaceFilesPanel({
  workspaceId,
  className,
}: WorkspaceFilesPanelProps) {
  const { theme } = useTheme();
  const selectedPath = useWorkspaceFilesStore((s) =>
    s.workspaceId === workspaceId ? s.selectedPath : null
  );
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
    if (!selectedPath) {
      setContent('');
      setSavedContent('');
      setFileError(null);
      return;
    }
    setFileError(null);
    setLoadingFile(true);
    workspaceFilesApi
      .read(workspaceId, selectedPath)
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
  }, [workspaceId, selectedPath]);

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
    <div className={cn('flex h-full min-h-0 flex-col bg-secondary', className)}>
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
              Select a file from the Files section in the sidebar
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
