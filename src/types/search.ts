export interface SearchResult {
  file_path: string;
  file_name: string;
  is_directory: boolean;
  score: number;
  match_source: "fts" | "vec";
  snippet: string | null;
}
