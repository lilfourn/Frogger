import { useState, useEffect } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import type { Extension } from "@codemirror/state";
import { useSettingsStore } from "../../stores/settingsStore";
import { readFileText } from "../../services/fileService";

const LANG_MAP: Record<string, () => Extension> = {
  js: () => javascript({ jsx: false }),
  jsx: () => javascript({ jsx: true }),
  ts: () => javascript({ jsx: false, typescript: true }),
  tsx: () => javascript({ jsx: true, typescript: true }),
  py: () => python(),
  rs: () => rust(),
  html: () => html(),
  css: () => css(),
  scss: () => css(),
  json: () => json(),
  md: () => markdown(),
  markdown: () => markdown(),
};

function getExtension(path: string): string {
  const parts = path.split(".");
  return parts.length > 1 ? parts[parts.length - 1].toLowerCase() : "";
}

export function CodePreview({ filePath }: { filePath: string }) {
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const resolvedTheme = useSettingsStore((s) => s.resolvedTheme);

  useEffect(() => {
    let cancelled = false;
    readFileText(filePath)
      .then((text) => {
        if (!cancelled) {
          setError(null);
          setContent(text);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setContent(null);
          setError(typeof e === "string" ? e : "Failed to read file");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [filePath]);

  if (error) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--color-text-secondary)]">
        {error}
      </div>
    );
  }

  if (content === null) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--color-text-secondary)]">
        Loading...
      </div>
    );
  }

  const ext = getExtension(filePath);
  const langFactory = LANG_MAP[ext];
  const extensions: Extension[] = langFactory ? [langFactory()] : [];

  return (
    <CodeMirror
      value={content}
      height="100%"
      readOnly={true}
      editable={false}
      theme={resolvedTheme() === "dark" ? "dark" : "light"}
      extensions={extensions}
      basicSetup={{
        lineNumbers: true,
        foldGutter: true,
        highlightActiveLine: false,
        highlightActiveLineGutter: false,
      }}
    />
  );
}
