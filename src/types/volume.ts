export interface VolumeInfo {
  name: string;
  path: string;
  total_bytes: number | null;
  free_bytes: number | null;
}
