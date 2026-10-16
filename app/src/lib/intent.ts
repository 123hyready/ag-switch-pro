import { invoke } from "@tauri-apps/api/core";

export interface ModelTarget {
  providerId: string;
  model: string;
}
export interface Worker extends ModelTarget {
  id: string;
  description: string;
  contextTokens: number;
  tools: boolean;
  images: boolean;
}
export type ModelCapabilities = Partial<
  Pick<Worker, "contextTokens" | "images">
>;
export interface DiscoveredModel {
  id: string;
  ownedBy: string | null;
  capabilities?: ModelCapabilities;
}
export interface RouteGroup {
  id: string;
  name: string;
  appType: "claude" | "codex";
  enabled: boolean;
  mode: "stage" | "turn" | "observe";
  judge: ModelTarget;
  workers: Worker[];
  defaultWorker: string;
  fallbackWorker: string;
  pinnedWorker: string | null;
  maxHoldRequests: number;
  maxJudgeCalls: number;
  judgeTimeoutMs: number;
  revision: number;
}
export interface DecisionRecord {
  id: string;
  createdAt: number;
  groupId: string;
  appType: string;
  sessionId: string | null;
  branch: string;
  phase: string;
  source: string;
  workerId: string;
  model: string;
  reason: string;
  judgeMillis: number;
  judgeInputTokens: number | null;
  judgeOutputTokens: number | null;
  judgeCostUsd: string | null;
  judgeModel?: string | null;
  decisionSource?: string | null;
  selectedModel?: string | null;
  recommendedModel?: string | null;
  executionAttempts?: {
    model: string;
    providerId: string;
    status: "running" | "accepted" | "failed";
    elapsedMs: number;
  }[];
}

export function virtualModelId(
  appType: RouteGroup["appType"],
  id: string,
): string {
  return appType === "claude" ? `ags-claude-group-${id}` : `ags-group/${id}`;
}

export function createRouteGroup(appType: RouteGroup["appType"]): RouteGroup {
  return {
    id: crypto.randomUUID(),
    name: "智能编码",
    appType,
    enabled: true,
    mode: "stage",
    judge: { providerId: "", model: "" },
    workers: [],
    defaultWorker: "",
    fallbackWorker: "",
    pinnedWorker: null,
    maxHoldRequests: 3,
    maxJudgeCalls: 20,
    judgeTimeoutMs: 8000,
    revision: 1,
  };
}

export function validateRouteGroup(group: RouteGroup): string | null {
  if (!group.name.trim()) return "请填写路由组名称";
  if (!group.judge.providerId || !group.judge.model.trim())
    return "请选择判断模型的供应商并填写模型名";
  if (!group.workers.length) return "至少添加一个工作模型";
  const ids = new Set<string>();
  for (const worker of group.workers) {
    if (!worker.providerId || !worker.model.trim())
      return "请完整配置每个工作模型";
    if (ids.has(worker.id)) return "工作模型 ID 重复";
    ids.add(worker.id);
    if (!Number.isInteger(worker.contextTokens) || worker.contextTokens < 1024)
      return "上下文容量必须为至少 1024 的整数";
  }
  if (!ids.has(group.defaultWorker)) return "请选择默认工作模型";
  if (!ids.has(group.fallbackWorker)) return "请选择回退工作模型";
  if (group.pinnedWorker && !ids.has(group.pinnedWorker))
    return "固定模型已被删除，请重新选择";
  if (
    !Number.isInteger(group.maxHoldRequests) ||
    group.maxHoldRequests < 1 ||
    group.maxHoldRequests > 20
  )
    return "连续请求数应为 1–20 的整数";
  if (
    !Number.isInteger(group.maxJudgeCalls) ||
    group.maxJudgeCalls < 0 ||
    group.maxJudgeCalls > 1000
  )
    return "判断预算应为 0–1000 次";
  if (
    !Number.isInteger(group.judgeTimeoutMs) ||
    group.judgeTimeoutMs < 100 ||
    group.judgeTimeoutMs > 30000
  )
    return "判断超时应为 100–30000 毫秒";
  return null;
}

export const intentApi = {
  models: (appType: string, providerId: string) =>
    invoke<DiscoveredModel[]>("get_intent_provider_models", {
      appType,
      providerId,
    }),
  list: () => invoke<RouteGroup[]>("list_intent_groups"),
  save: (group: RouteGroup) =>
    invoke<RouteGroup>("save_intent_group", { group }),
  remove: (id: string) => invoke<void>("delete_intent_group", { id }),
  records: () =>
    invoke<DecisionRecord[]>("list_intent_decisions", { limit: 100 }),
  simulate: (group: RouteGroup, text: string) =>
    invoke<DecisionRecord>("simulate_intent_route", { group, text }),
};
