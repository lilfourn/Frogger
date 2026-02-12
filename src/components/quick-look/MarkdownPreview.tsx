import { useState, useEffect } from "react";
import Markdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import { readFileText } from "../../services/fileService";

export function MarkdownPreview({ filePath }: { filePath: string }) {
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <div className="prose prose-sm h-full max-w-none overflow-y-auto p-6 text-[var(--color-text)] dark:prose-invert">
      <Markdown rehypePlugins={[rehypeSanitize]}>{content}</Markdown>
    </div>
  );
}
