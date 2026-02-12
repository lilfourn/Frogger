export function AudioPreview({ filePath }: { filePath: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 p-6">
      <audio src={`asset://localhost/${filePath}`} controls className="w-full max-w-md" />
    </div>
  );
}
