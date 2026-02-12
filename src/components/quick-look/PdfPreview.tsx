import { useState, useEffect, useRef } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import * as pdfjsLib from "pdfjs-dist";

pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
  "pdfjs-dist/build/pdf.worker.min.mjs",
  import.meta.url,
).toString();

export function PdfPreview({ filePath }: { filePath: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [pageNum, setPageNum] = useState(1);
  const [numPages, setNumPages] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const pdfDocRef = useRef<pdfjsLib.PDFDocumentProxy | null>(null);

  useEffect(() => {
    let cancelled = false;
    pdfDocRef.current = null;
    const url = `asset://localhost/${filePath}`;
    pdfjsLib
      .getDocument(url)
      .promise.then((pdf) => {
        if (cancelled) return;
        pdfDocRef.current = pdf;
        setError(null);
        setPageNum(1);
        setNumPages(pdf.numPages);
      })
      .catch((err) => {
        console.error("[PdfPreview] Failed to load PDF:", err);
        if (!cancelled) setError("Failed to load PDF");
      });
    return () => {
      cancelled = true;
    };
  }, [filePath]);

  useEffect(() => {
    if (numPages <= 0) return;
    let cancelled = false;
    const pdf = pdfDocRef.current;
    const canvas = canvasRef.current;
    if (!pdf || !canvas) return;

    pdf
      .getPage(pageNum)
      .then((page) => {
        if (cancelled) return;
        const scale = 1.5;
        const viewport = page.getViewport({ scale });
        const outputScale = window.devicePixelRatio || 1;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        canvas.width = Math.floor(viewport.width * outputScale);
        canvas.height = Math.floor(viewport.height * outputScale);
        canvas.style.width = `${Math.floor(viewport.width)}px`;
        canvas.style.height = `${Math.floor(viewport.height)}px`;

        const transform = outputScale !== 1 ? [outputScale, 0, 0, outputScale, 0, 0] : undefined;

        return page.render({
          canvasContext: ctx,
          canvas,
          transform: transform as number[] | undefined,
          viewport,
        }).promise;
      })
      .catch((err) => {
        console.error("[PdfPreview] Failed to render page:", err);
        if (!cancelled) setError("Failed to render page");
      });

    return () => {
      cancelled = true;
    };
  }, [pageNum, numPages]);

  if (error) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--color-text-secondary)]">
        {error}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {numPages > 1 && (
        <div className="flex items-center justify-center gap-3 border-b border-[var(--color-border)] py-1.5">
          <button
            onClick={() => setPageNum((p) => Math.max(1, p - 1))}
            disabled={pageNum <= 1}
            className="rounded p-1 text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)] disabled:opacity-30"
          >
            <ChevronLeft size={16} />
          </button>
          <span className="text-xs text-[var(--color-text-secondary)]">
            {pageNum} / {numPages}
          </span>
          <button
            onClick={() => setPageNum((p) => Math.min(numPages, p + 1))}
            disabled={pageNum >= numPages}
            className="rounded p-1 text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)] disabled:opacity-30"
          >
            <ChevronRight size={16} />
          </button>
        </div>
      )}
      <div className="flex flex-1 items-start justify-center overflow-auto p-4">
        <canvas ref={canvasRef} />
      </div>
    </div>
  );
}
