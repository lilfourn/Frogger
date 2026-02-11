export interface FileEntry {
  path: string;
  name: string;
  extension: string | null;
  mime_type: string | null;
  size_bytes: number | null;
  created_at: string | null;
  modified_at: string | null;
  is_directory: boolean;
  parent_path: string | null;
}
