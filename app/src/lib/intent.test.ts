import { describe, expect, it, vi } from "vitest";
import { createRouteGroup, validateRouteGroup, virtualModelId } from "./intent";
import { intentApi } from "./intent";
import { invoke } from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("intent route configuration", () => {
  it("creates an isolated group with one judge and an explicit fallback", () => {
    const group = createRouteGroup("codex");
    expect(group.mode).toBe("stage");
    expect(group.maxHoldRequests).toBe(3);
    expect(validateRouteGroup(group)).toContain("判断模型");
  });

  it("rejects invalid model targets and defaults", () => {
    const group = createRouteGroup("claude");
    group.judge = { providerId: "judge", model: "small" };
    group.workers = [
      {
        id: "w1",
        providerId: "p",
        model: "model",
        description: "Coding",
        contextTokens: 32000,
        tools: true,
        images: false,
      },
    ];
    expect(validateRouteGroup(group)).toContain("默认");
    group.defaultWorker = "w1";
    group.fallbackWorker = "w1";
    expect(validateRouteGroup(group)).toBeNull();
    group.workers[0].contextTokens = 0;
    expect(validateRouteGroup(group)).toContain("上下文");
  });

  it("publishes separate aliases for each client", () => {
    expect(virtualModelId("codex", "coding")).toBe("ags-group/coding");
    expect(virtualModelId("claude", "coding")).toBe("ags-claude-group-coding");
  });

  it("rejects stale pins, duplicates and out-of-range judge settings", () => {
    const group = createRouteGroup("codex");
    group.judge = { providerId: "p", model: "judge" };
    group.workers = [
      {
        id: "w",
        providerId: "p",
        model: "worker",
        description: "",
        contextTokens: 32000,
        tools: true,
        images: false,
      },
    ];
    group.defaultWorker = "w";
    group.fallbackWorker = "w";
    group.name = "";
    expect(validateRouteGroup(group)).toContain("名称");
    group.name = "Coding";
    group.workers.push({ ...group.workers[0] });
    expect(validateRouteGroup(group)).toContain("重复");
    group.workers.pop();
    group.workers[0].model = "";
    expect(validateRouteGroup(group)).toContain("完整");
    group.workers[0].model = "worker";
    group.pinnedWorker = "missing";
    expect(validateRouteGroup(group)).toContain("固定");
    group.pinnedWorker = null;
    group.maxHoldRequests = 21;
    expect(validateRouteGroup(group)).toContain("连续");
    group.maxHoldRequests = 3;
    group.maxJudgeCalls = -1;
    expect(validateRouteGroup(group)).toContain("预算");
    group.maxJudgeCalls = 20;
    group.judgeTimeoutMs = 1;
    expect(validateRouteGroup(group)).toContain("超时");
  });

  it("uses the desktop command contracts without exposing credentials", async () => {
    const group = createRouteGroup("codex");
    vi.mocked(invoke).mockResolvedValue(group);
    expect(await intentApi.save(group)).toBe(group);
    expect(invoke).toHaveBeenLastCalledWith("save_intent_group", { group });
    await intentApi.list();
    expect(invoke).toHaveBeenLastCalledWith("list_intent_groups");
    await intentApi.records();
    expect(invoke).toHaveBeenLastCalledWith("list_intent_decisions", {
      limit: 100,
    });
    await intentApi.simulate(group, "Task");
    expect(invoke).toHaveBeenLastCalledWith("simulate_intent_route", {
      group,
      text: "Task",
    });
    await intentApi.remove(group.id);
    expect(invoke).toHaveBeenLastCalledWith("delete_intent_group", {
      id: group.id,
    });
  });
});
