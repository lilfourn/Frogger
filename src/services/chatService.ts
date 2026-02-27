import { invoke } from "@tauri-apps/api/core";
import { preflightPermission, retryPermissionAfterFailure } from "./permissionGate";

export interface ChatRecord {
  id: number;
  session_id: string;
  role: string;
  content: string;
  created_at: string;
}

export interface OrganizeProgressPayload {
  sessionId: string;
  rootPath: string;
  phase: "indexing" | "planning" | "applying" | "done" | "cancelled" | "error";
  processed: number;
  total: number;
  percent: number;
  combinedPercent: number;
  message: string;
  sequence: number;
}

export async function sendChat(
  message: string,
  sessionId: string,
  currentDir: string,
  selectedFiles: string[],
): Promise<string> {
  const promptTitle = "Share file context with chat assistant";
  const allowOnce = await preflightPermission(
    "send_chat",
    [currentDir, ...selectedFiles],
    promptTitle,
  );
  try {
    return await invoke<string>("send_chat", {
      message,
      sessionId,
      currentDir,
      selectedFiles,
      allowOnce,
    });
  } catch (error) {
    if (
      !allowOnce &&
      (await retryPermissionAfterFailure("send_chat", [currentDir, ...selectedFiles], promptTitle))
    ) {
      return invoke<string>("send_chat", {
        message,
        sessionId,
        currentDir,
        selectedFiles,
        allowOnce: true,
      });
    }
    throw error;
  }
}

export async function getChatHistory(sessionId: string): Promise<ChatRecord[]> {
  return invoke<ChatRecord[]>("get_chat_history", { sessionId });
}

export async function clearChatHistory(sessionId: string): Promise<void> {
  return invoke("clear_chat_history", { sessionId });
}

export async function sendOrganizePlan(
  currentDir: string,
  organizeSessionId?: string,
): Promise<string> {
  const promptTitle = "Share directory context for organization plan";
  const allowOnce = await preflightPermission("send_organize_plan", [currentDir], promptTitle);
  try {
    return await invoke<string>("send_organize_plan", { currentDir, allowOnce, organizeSessionId });
  } catch (error) {
    if (
      !allowOnce &&
      (await retryPermissionAfterFailure("send_organize_plan", [currentDir], promptTitle))
    ) {
      return invoke<string>("send_organize_plan", {
        currentDir,
        allowOnce: true,
        organizeSessionId,
      });
    }
    throw error;
  }
}

export async function sendOrganizeExecute(
  currentDir: string,
  planJson: string,
  organizeSessionId?: string,
): Promise<string> {
  const promptTitle = "Share directory context for organization actions";
  const allowOnce = await preflightPermission("send_organize_execute", [currentDir], promptTitle);
  try {
    return await invoke<string>("send_organize_execute", {
      currentDir,
      planJson,
      allowOnce,
      organizeSessionId,
    });
  } catch (error) {
    if (
      !allowOnce &&
      (await retryPermissionAfterFailure("send_organize_execute", [currentDir], promptTitle))
    ) {
      return invoke<string>("send_organize_execute", {
        currentDir,
        planJson,
        allowOnce: true,
        organizeSessionId,
      });
    }
    throw error;
  }
}

export async function sendOrganizeApply(
  currentDir: string,
  planJson: string,
  organizeSessionId?: string,
): Promise<string> {
  const promptTitle = "Apply AI organization file operations";
  const allowOnce = await preflightPermission("send_organize_apply", [currentDir], promptTitle);
  try {
    return await invoke<string>("send_organize_apply", {
      currentDir,
      planJson,
      allowOnce,
      organizeSessionId,
    });
  } catch (error) {
    if (
      !allowOnce &&
      (await retryPermissionAfterFailure("send_organize_apply", [currentDir], promptTitle))
    ) {
      return invoke<string>("send_organize_apply", {
        currentDir,
        planJson,
        allowOnce: true,
        organizeSessionId,
      });
    }
    throw error;
  }
}

export async function cancelOrganize(
  currentDir: string,
  organizeSessionId?: string,
): Promise<void> {
  return invoke("cancel_organize", { currentDir, organizeSessionId });
}

export async function getOrganizeStatus(
  organizeSessionId: string,
): Promise<OrganizeProgressPayload | null> {
  return invoke<OrganizeProgressPayload | null>("get_organize_status", { organizeSessionId });
}

export async function newChatSession(): Promise<string> {
  return invoke<string>("new_chat_session");
}
