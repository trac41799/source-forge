import * as React from "react";
import { open, type DialogFilter, type OpenDialogOptions } from "@tauri-apps/plugin-dialog";
import { FileText, FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useProjectStore } from "@/stores/projectStore";

/** Opens a native Tauri dialog and normalizes single/multi selection to a string path. */
async function pickDialogPath(options: OpenDialogOptions): Promise<string | null> {
  try {
    const selected = await open(options);
    if (typeof selected === "string" && selected.length > 0) {
      return selected;
    }
    if (Array.isArray(selected) && selected.length > 0) {
      return selected[0];
    }
    return null;
  } catch (err) {
    console.error("Failed to open path dialog:", err);
    return null;
  }
}

export interface FolderPickerInputProps {
  value: string;
  onChange: (value: string) => void;
  id?: string;
  placeholder?: string;
  title?: string;
  defaultPath?: string;
  browseLabel?: string;
  recentPaths?: string[];
  disabled?: boolean;
  className?: string;
  inputClassName?: string;
  ariaLabel?: string;
}

/**
 * Folder path input paired with an optional recent-projects quick-select and native OS folder picker.
 */
export function FolderPickerInput({
  value,
  onChange,
  id,
  placeholder = "/path/to/project",
  title = "Select Folder",
  defaultPath,
  browseLabel = "Browse...",
  recentPaths,
  disabled = false,
  className,
  inputClassName,
  ariaLabel,
}: FolderPickerInputProps) {
  const storeRecentPaths = useProjectStore((s) => s.recentPaths);
  const currentProjectPath = useProjectStore((s) => s.currentProject?.path);

  const folderOptions = React.useMemo(() => {
    const raw = recentPaths ?? [
      ...(currentProjectPath ? [currentProjectPath] : []),
      ...storeRecentPaths,
    ];
    return Array.from(new Set(raw.filter((p) => Boolean(p && p.trim()))));
  }, [recentPaths, currentProjectPath, storeRecentPaths]);

  const handleBrowse = async () => {
    const picked = await pickDialogPath({
      directory: true,
      multiple: false,
      title,
      ...(defaultPath ? { defaultPath } : {}),
    });
    if (picked) {
      onChange(picked);
    }
  };

  return (
    <div className={cn("flex items-center gap-2", className)}>
      <Input
        id={id}
        aria-label={ariaLabel}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        className={cn("flex-1 min-w-0", inputClassName)}
      />
      {folderOptions.length > 0 && (
        <select
          aria-label="Recent folders"
          disabled={disabled}
          value={folderOptions.includes(value) ? value : ""}
          onChange={(e) => {
            if (e.target.value) onChange(e.target.value);
          }}
          className="h-9 max-w-[160px] shrink-0 rounded-lg border border-glass-border bg-glass-20 px-2.5 text-xs text-foreground outline-none focus:border-indigo-500/40"
        >
          <option value="">Recent...</option>
          {folderOptions.map((folder) => (
            <option key={folder} value={folder}>
              {folder}
            </option>
          ))}
        </select>
      )}
      <Button
        type="button"
        variant="outline"
        disabled={disabled}
        onClick={() => void handleBrowse()}
        className="shrink-0 gap-1.5"
      >
        <FolderOpen className="size-4" />
        {browseLabel}
      </Button>
    </div>
  );
}

export interface FilePickerInputProps {
  value: string;
  onChange: (value: string) => void;
  id?: string;
  placeholder?: string;
  title?: string;
  filters?: DialogFilter[];
  defaultPath?: string;
  browseLabel?: string;
  disabled?: boolean;
  className?: string;
  inputClassName?: string;
  ariaLabel?: string;
}

/**
 * File path input paired with a native OS file picker dialog button.
 */
export function FilePickerInput({
  value,
  onChange,
  id,
  placeholder = "/path/to/file",
  title = "Select File",
  filters,
  defaultPath,
  browseLabel = "Browse...",
  disabled = false,
  className,
  inputClassName,
  ariaLabel,
}: FilePickerInputProps) {
  const handleBrowse = async () => {
    const picked = await pickDialogPath({
      directory: false,
      multiple: false,
      title,
      ...(filters ? { filters } : {}),
      ...(defaultPath ? { defaultPath } : {}),
    });
    if (picked) {
      onChange(picked);
    }
  };

  return (
    <div className={cn("flex items-center gap-2", className)}>
      <Input
        id={id}
        aria-label={ariaLabel}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        className={cn("flex-1 min-w-0", inputClassName)}
      />
      <Button
        type="button"
        variant="outline"
        disabled={disabled}
        onClick={() => void handleBrowse()}
        className="shrink-0 gap-1.5"
      >
        <FileText className="size-4" />
        {browseLabel}
      </Button>
    </div>
  );
}
