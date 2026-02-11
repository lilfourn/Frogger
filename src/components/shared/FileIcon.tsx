import folderIcon from "../../assets/folder-icon.svg";
import fileIcon from "../../assets/file-icon.svg";

interface FileIconProps {
  isDirectory: boolean;
  size?: number;
}

export function FileIcon({ isDirectory, size = 16 }: FileIconProps) {
  return (
    <img
      src={isDirectory ? folderIcon : fileIcon}
      alt={isDirectory ? "folder" : "file"}
      width={size}
      height={size}
      className="shrink-0"
    />
  );
}
