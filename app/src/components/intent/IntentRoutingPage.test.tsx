import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { IntentRoutingPage } from "./IntentRoutingPage";
import { createRouteGroup, intentApi } from "@/lib/intent";
import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import { toast } from "@/lib/toast";

vi.mock("@/lib/intent", async (load) => ({
  ...(await load<typeof import("@/lib/intent")>()),
  intentApi: {
    list: vi.fn(),
    models: vi.fn().mockResolvedValue([]),
    records: vi.fn(),
    save: vi.fn(),
    remove: vi.fn(),
    simulate: vi.fn(),
  },
}));
vi.mock("@/lib/api/providers", () => ({ providersApi: { getAll: vi.fn() } }));
vi.mock("@/lib/api/proxy", () => ({
  proxyApi: { setProxyTakeoverForApp: vi.fn() },
}));
vi.mock("@/components/shell/AppPageHeader", () => ({
  AppPageHeader: ({ title }: { title: string }) => <h1>{title}</h1>,
}));
vi.mock("@/lib/toast", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(intentApi.list).mockResolvedValue([]);
  vi.mocked(intentApi.records).mockResolvedValue([]);
  vi.mocked(providersApi.getAll).mockResolvedValue({
    provider: { id: "provider", name: "Local provider", settingsConfig: {} },
  });
});

describe("intent routing settings", () => {
  it("requires configured judge and workers before saving", async () => {
    render(<IntentRoutingPage />);
    await screen.findByText("尚无路由记录");
    fireEvent.click(screen.getByText("保存路由组"));
    expect(intentApi.save).not.toHaveBeenCalled();
  });

  it("saves a configured group with explicit model capabilities", async () => {
    const saved = createRouteGroup("codex");
    vi.mocked(intentApi.save).mockImplementation(async (group) => ({
      ...group,
      revision: 2,
    }));
    render(<IntentRoutingPage />);
    await waitFor(() =>
      expect(providersApi.getAll).toHaveBeenCalledWith("codex"),
    );
    fireEvent.change(screen.getByLabelText("判断模型供应商"), {
      target: { value: "provider" },
    });
    fireEvent.change(screen.getByLabelText("判断模型名称"), {
      target: { value: "small-judge" },
    });
    fireEvent.click(screen.getByText("添加工作模型"));
    fireEvent.change(screen.getByLabelText("工作模型 1 供应商"), {
      target: { value: "provider" },
    });
    fireEvent.change(screen.getByLabelText("工作模型 1 名称"), {
      target: { value: "coding-model" },
    });
    fireEvent.click(screen.getByLabelText("图片输入"));
    fireEvent.change(screen.getByLabelText("连续请求数上限"), {
      target: { value: "5" },
    });
    fireEvent.change(screen.getByLabelText("判断次数预算"), {
      target: { value: "12" },
    });
    fireEvent.change(screen.getByLabelText("判断超时"), {
      target: { value: "3000" },
    });
    fireEvent.click(screen.getByText("保存路由组"));
    await waitFor(() => expect(intentApi.save).toHaveBeenCalledTimes(1));
    const submitted = vi.mocked(intentApi.save).mock.calls[0][0];
    expect(submitted.mode).toBe(saved.mode);
    expect(submitted.workers[0].tools).toBe(true);
    expect(submitted.defaultWorker).toBe(submitted.workers[0].id);
    expect(submitted.workers[0].images).toBe(true);
    expect(submitted.maxHoldRequests).toBe(5);
    expect(submitted.maxJudgeCalls).toBe(12);
  });

  it("shows unknown costs without presenting them as free", async () => {
    vi.mocked(intentApi.records).mockResolvedValue([
      {
        id: "r1",
        createdAt: 1,
        groupId: "g",
        appType: "codex",
        sessionId: null,
        branch: "main",
        phase: "collect",
        source: "judge",
        workerId: "w",
        model: "cheap-model",
        reason: "Read files",
        judgeMillis: 10,
        judgeInputTokens: 20,
        judgeOutputTokens: 10,
        judgeCostUsd: null,
      },
    ]);
    render(<IntentRoutingPage />);
    expect((await screen.findAllByText("cheap-model")).length).toBeGreaterThan(0);
    expect(screen.getByText("未知 / 未调用")).toBeInTheDocument();
  });

  it("starts a saved group, simulates and restores the client without automatic takeover", async () => {
    const group = createRouteGroup("codex");
    group.judge = { providerId: "provider", model: "judge" };
    group.workers = [
      {
        id: "w",
        providerId: "provider",
        model: "worker",
        description: "Coding",
        contextTokens: 128000,
        tools: true,
        images: false,
      },
    ];
    group.defaultWorker = "w";
    group.fallbackWorker = "w";
    vi.mocked(intentApi.list).mockResolvedValue([group]);
    vi.mocked(intentApi.simulate).mockResolvedValue({
      id: "sim",
      createdAt: 1,
      groupId: group.id,
      appType: "codex",
      sessionId: null,
      branch: "simulation",
      phase: "collect",
      source: "judge",
      workerId: "w",
      model: "worker",
      reason: "Mechanical task",
      judgeMillis: 10,
      judgeInputTokens: 20,
      judgeOutputTokens: 10,
      judgeCostUsd: "0.001",
    });
    vi.mocked(proxyApi.setProxyTakeoverForApp).mockResolvedValue();
    render(<IntentRoutingPage />);
    fireEvent.click(await screen.findByText(group.name));
    expect(proxyApi.setProxyTakeoverForApp).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("启动客户端路由"));
    await waitFor(() =>
      expect(proxyApi.setProxyTakeoverForApp).toHaveBeenCalledWith(
        "codex",
        true,
        false,
        "provider",
      ),
    );
    await waitFor(() =>
      expect(screen.getByText("启动客户端路由")).not.toBeDisabled(),
    );
    fireEvent.change(screen.getByLabelText("试判断任务"), {
      target: { value: "Read the file" },
    });
    fireEvent.click(screen.getByText("判断一次"));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Mechanical task",
    );
    await waitFor(() =>
      expect(screen.getByText("恢复直连")).not.toBeDisabled(),
    );
    fireEvent.click(screen.getByText("恢复直连"));
    await waitFor(() =>
      expect(proxyApi.setProxyTakeoverForApp).toHaveBeenLastCalledWith(
        "codex",
        false,
      ),
    );
    await waitFor(() =>
      expect(screen.getByText("删除路由组")).not.toBeDisabled(),
    );
    fireEvent.click(screen.getByText("删除路由组"));
    await waitFor(() =>
      expect(intentApi.remove).toHaveBeenCalledWith(group.id),
    );
  });

  it("blocks unsaved takeover and edits worker settings", async () => {
    await act(async () => {
      render(<IntentRoutingPage />);
    });
    fireEvent.click(screen.getByText("启动客户端路由"));
    expect(proxyApi.setProxyTakeoverForApp).not.toHaveBeenCalled();
    expect(toast.error).toHaveBeenCalledWith("请先保存当前配置");
    fireEvent.click(screen.getByText("添加工作模型"));
    fireEvent.change(screen.getByLabelText("工作模型 1 能力说明"), {
      target: { value: "Cheap" },
    });
    fireEvent.change(screen.getByLabelText("工作模型 1 上下文容量"), {
      target: { value: "32000" },
    });
    fireEvent.click(screen.getByLabelText("删除工作模型 1"));
    expect(screen.queryByLabelText("工作模型 1 名称")).not.toBeInTheDocument();
    await act(async () => {
      fireEvent.change(screen.getByLabelText("客户端"), {
        target: { value: "claude" },
      });
    });
    expect(screen.getByLabelText("客户端")).toHaveValue("claude");
  });
});
