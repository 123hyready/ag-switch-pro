import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { DecisionRecord } from "@/lib/intent";
import { RouteTrace } from "./RouteTrace";

const record: DecisionRecord = {
  id: "request", createdAt: 1, groupId: "group", appType: "codex",
  sessionId: "session", branch: "main", phase: "implement", source: "judge",
  workerId: "worker", model: "sol", reason: "Complex implementation",
  judgeMillis: 400, judgeInputTokens: 20, judgeOutputTokens: 10,
  judgeCostUsd: null, judgeModel: "astra", selectedModel: "luna",
};

describe("route trace", () => {
  it("preserves judge and initial choice through an execution fallback", () => {
    render(<RouteTrace record={{ ...record, source: "execution_fallback", decisionSource: "judge",
      executionAttempts: [
        { model: "luna", providerId: "official", status: "failed", elapsedMs: 500 },
        { model: "sol", providerId: "official", status: "accepted", elapsedMs: 700 },
      ],
    }} />);
    expect(screen.getByText(/astra · 已判断/)).toBeInTheDocument();
    expect(screen.getByText(/luna · 请求失败/)).toBeInTheDocument();
    expect(screen.getByText(/回退 → sol · 上游已接收/)).toBeInTheDocument();
    expect(screen.getByText(/不代表输出已完成/)).toBeInTheDocument();
  });
  it("does not claim a new judge call when reusing a decision", () => {
    render(<RouteTrace record={{ ...record, source: "hold" }} />);
    expect(screen.getByText(/本次未调用判断模型（配置：astra）/)).toBeInTheDocument();
  });
  it("does not invent judge or execution evidence for legacy records", () => {
    render(<RouteTrace record={{ ...record, judgeModel: undefined }} />);
    expect(screen.getByText(/历史记录未保存判断模型/)).toBeInTheDocument();
    expect(screen.getByText(/尚无执行记录/)).toBeInTheDocument();
  });
});
