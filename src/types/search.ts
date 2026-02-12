export interface SearchResult {
  file_path: string;
  file_name: string;
  score: number;
  match_source: "fts" | "vec" | "hybrid";
  snippet: string | null;
}
