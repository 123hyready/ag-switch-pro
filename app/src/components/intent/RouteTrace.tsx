import type { DecisionRecord } from "@/lib/intent";

const statusNames = {
  running: "正在请求上游（尚未记录结果）",
  accepted: "上游已接收（不代表输出已完成）",
  failed: "请求失败",
};

/** Historical records deliberately do not borrow today's judge configuration. */
export function RouteTrace({ record }: { record: DecisionRecord }) {
  const source = record.decisionSource ?? record.source;
  const judged = ["judge", "observe", "fallback"].includes(source);
  return (
    <div className="min-w-80 space-y-2 py-1">
      <div>
        <strong>① 判断：</strong>
        {!record.judgeModel
          ? "历史记录未保存判断模型"
          : judged
            ? `${record.judgeModel} · ${source === "fallback" ? "判断未成功，使用回退" : "已判断"}`
            : `本次未调用判断模型（配置：${record.judgeModel}）`}
        {judged && <span> · {record.judgeMillis} ms</span>}
      </div>
      <div>
        <strong>② 选择：</strong>{record.selectedModel ?? record.model}
        {record.recommendedModel && record.recommendedModel !== record.selectedModel && (
          <span>（判断建议：{record.recommendedModel}）</span>
        )}
        <p className="text-muted-foreground">{record.reason}</p>
      </div>
      <div>
        <strong>③ 执行：</strong>
        {!record.executionAttempts?.length
          ? "尚无执行记录（试判断不会执行任务）"
          : record.executionAttempts.map((attempt, index) => (
              <p key={index}>
                {index > 0 ? "回退 → " : "→ "}{attempt.model} · {statusNames[attempt.status]} · {attempt.elapsedMs} ms
                <span className="text-muted-foreground"> · {attempt.providerId}</span>
              </p>
            ))}
      </div>
      <details className="text-xs text-muted-foreground">
        <summary>请求信息 · {new Date(record.createdAt).toLocaleString()}</summary>
        <p>记录：{record.id}</p>
        <p>路由组：{record.groupId} · 会话：{record.sessionId ?? "未提供"}</p>
      </details>
    </div>
  );
}
