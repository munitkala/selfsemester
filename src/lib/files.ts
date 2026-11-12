// Small helpers for showing attached documents.

const KINDS: [RegExp, string][] = [
  [/\.pdf$/i, "PDF"],
  [/\.(docx?|odt|rtf)$/i, "Doc"],
  [/\.(pptx?|odp|key)$/i, "Slides"],
  [/\.(xlsx?|ods|csv)$/i, "Sheet"],
  [/\.(md|markdown|txt)$/i, "Text"],
  [/\.(png|jpe?g|webp|gif|svg)$/i, "Image"],
  [/\.(mp3|m4a|wav)$/i, "Audio"],
  [/\.(mp4|webm)$/i, "Video"],
  [/\.epub$/i, "Book"],
];

export function fileKind(name: string): string {
  return KINDS.find(([re]) => re.test(name))?.[1] ?? "File";
}

export function fileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
