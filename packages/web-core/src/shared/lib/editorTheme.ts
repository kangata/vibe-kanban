import type { Extension } from '@uiw/react-codemirror';
import { createTheme } from '@uiw/codemirror-themes';
import { tags as t } from '@lezer/highlight';

import { getCssVariable, hslToHex } from './terminalTheme';

/**
 * Build a CodeMirror theme from the app's CSS variables so the editor
 * matches the surrounding UI in both light and dark mode. The syntax
 * palette mirrors the terminal's ANSI colors for consistency.
 */
export function getEditorTheme(): Extension {
  const background = hslToHex(getCssVariable('--bg-secondary'));
  const foreground = hslToHex(getCssVariable('--text-high'));
  const muted = hslToHex(getCssVariable('--text-low'));
  const success = hslToHex(getCssVariable('--console-success'));
  const error = hslToHex(getCssVariable('--console-error'));

  const isDark = document.documentElement.classList.contains('dark');

  if (isDark) {
    return createTheme({
      theme: 'dark',
      settings: {
        background,
        foreground,
        caret: foreground,
        selection: '#3d4966',
        selectionMatch: '#3d496680',
        lineHighlight: '#ffffff0a',
        gutterBackground: background,
        gutterForeground: muted,
        gutterBorder: 'transparent',
      },
      styles: [
        { tag: [t.keyword, t.modifier, t.operatorKeyword], color: '#bb9af7' },
        { tag: [t.string, t.special(t.string)], color: success },
        { tag: [t.number, t.bool, t.null, t.atom], color: '#e0af68' },
        { tag: [t.comment, t.blockComment, t.lineComment], color: '#545c7e' },
        {
          tag: [t.function(t.variableName), t.function(t.propertyName)],
          color: '#7aa2f7',
        },
        { tag: [t.typeName, t.className, t.namespace], color: '#7dcfff' },
        { tag: [t.tagName, t.angleBracket], color: '#7aa2f7' },
        { tag: [t.propertyName, t.attributeName], color: '#73daca' },
        { tag: [t.definition(t.variableName)], color: foreground },
        { tag: [t.invalid], color: error },
        { tag: [t.heading], color: '#7aa2f7', fontWeight: 'bold' },
        { tag: [t.link, t.url], color: '#7dcfff' },
      ],
    });
  }

  return createTheme({
    theme: 'light',
    settings: {
      background,
      foreground,
      caret: foreground,
      selection: '#accef7',
      selectionMatch: '#accef780',
      lineHighlight: '#00000008',
      gutterBackground: background,
      gutterForeground: muted,
      gutterBorder: 'transparent',
    },
    styles: [
      { tag: [t.keyword, t.modifier, t.operatorKeyword], color: '#8250df' },
      { tag: [t.string, t.special(t.string)], color: success },
      { tag: [t.number, t.bool, t.null, t.atom], color: '#946800' },
      { tag: [t.comment, t.blockComment, t.lineComment], color: '#6e7781' },
      {
        tag: [t.function(t.variableName), t.function(t.propertyName)],
        color: '#0969da',
      },
      { tag: [t.typeName, t.className, t.namespace], color: '#0e7490' },
      { tag: [t.tagName, t.angleBracket], color: '#0550ae' },
      { tag: [t.propertyName, t.attributeName], color: '#0e7490' },
      { tag: [t.definition(t.variableName)], color: foreground },
      { tag: [t.invalid], color: error },
      { tag: [t.heading], color: '#0550ae', fontWeight: 'bold' },
      { tag: [t.link, t.url], color: '#0969da' },
    ],
  });
}
