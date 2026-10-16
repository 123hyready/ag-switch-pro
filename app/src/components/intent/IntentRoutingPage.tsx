import { useEffect, useState } from "react";
import { Plus, Route, Trash2 } from "lucide-react";
import { AppPageHeader } from "@/components/shell/AppPageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import { toast } from "@/lib/toast";
import {
  createRouteGroup,
  intentApi,
  validateRouteGroup,
  virtualModelId,
  type DecisionRecord,
  type RouteGroup,
  type Worker,
} from "@/lib/intent";
import { ModelPicker } from "./ModelPicker";
import { RouteTrace } from "./RouteTrace";
import type { Provider } from "@/types";

const selectStyle =
  "h-9 w-full rounded-md border border-input bg-background px-3 text-sm";
const sourceNames: Record<string, string> = {
  judge: "模型判断",
  hold: "阶段保持",
  duplicate: "重复请求",
  fallback: "判断回退",
  budget: "判断次数已达上限（使用回退模型）",
  pinned: "手动固定",
  continuity: "保持供应商状态",
  observe: "观察模式",
  paused: "已暂停",
  execution_fallback: "执行回退",
};

function ProviderSelect({
  value,
  onChange,
  providers,
  label,
}: {
  value: string;
  onChange: (id: string) => void;
  providers: Provider[];
  label: string;
}) {
  return (
    <label className="grid gap-1 text-sm">
      {label}
      <select
        aria-label={label}
        className={selectStyle}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        <option value="">选择供应商</option>
        {providers.map((provider) => (
          <option key={provider.id} value={provider.id}>
            {provider.name}
          </option>
        ))}
      </select>
      {providers.find((provider) => provider.id === value)?.category ===
        "official" && (
        <span className="text-xs text-muted-foreground">
          Codex 官方供应商未绑定托管账号时，使用当前 Codex 登录；需先完成 codex
          login。其他客户端的原生官方登录暂不支持路由。
        </span>
      )}
    </label>
  );
}

export function IntentRoutingPage() {
  const [groups, setGroups] = useState<RouteGroup[]>([]);
  const [draft, setDraft] = useState<RouteGroup>(() =>
    createRouteGroup("codex"),
  );
  const [providers, setProviders] = useState<Provider[]>([]);
  const [records, setRecords] = useState<DecisionRecord[]>([]);
  const [busy, setBusy] = useState(false);
  const [simulationText, setSimulationText] = useState("");
  const [simulation, setSimulation] = useState<DecisionRecord | null>(null);

  async function refresh() {
    const [saved, history] = await Promise.all([
      intentApi.list(),
      intentApi.records(),
    ]);
    setGroups(saved);
    setRecords(history);
  }

  useEffect(() => {
    refresh().catch((error) => toast.error(String(error)));
  }, []);
  useEffect(() => {
    let active = true;
    let pending = false;
    const timer = window.setInterval(async () => {
      if (pending) return;
      pending = true;
      try {
        const history = await intentApi.records();
        if (active) setRecords(history);
      } catch (error) {
        console.warn("Unable to refresh intent route history", error);
      } finally {
        pending = false;
      }
    }, 5000);
    return () => { active = false; window.clearInterval(timer); };
  }, []);
  useEffect(() => {
    let active = true;
    providersApi
      .getAll(draft.appType)
      .then((found) => {
        if (active) setProviders(Object.values(found));
      })
      .catch((error) => {
        if (active) toast.error(String(error));
      });
    return () => {
      active = false;
    };
  }, [draft.appType]);

  function changeWorker(id: string, patch: Partial<Worker>) {
    setDraft((previous) => ({
      ...previous,
      workers: previous.workers.map((worker) =>
        worker.id === id ? { ...worker, ...patch } : worker,
      ),
    }));
  }

  function addWorker() {
    const id = crypto.randomUUID();
    setDraft((previous) => ({
      ...previous,
      workers: [
        ...previous.workers,
        {
          id,
          providerId: "",
          model: "",
          description: "",
          contextTokens: 128000,
          tools: true,
          images: false,
        },
      ],
      defaultWorker: previous.defaultWorker || id,
      fallbackWorker: previous.fallbackWorker || id,
    }));
  }

  async function run(action: () => Promise<void>) {
    setBusy(true);
    try {
      await action();
    } catch (error) {
      toast.error(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function save() {
    const problem = validateRouteGroup(draft);
    if (problem) {
      toast.error(problem);
      return;
    }
    await run(async () => {
      const saved = await intentApi.save(draft);
      setDraft(saved);
      await refresh();
      toast.success("路由组已保存");
    });
  }

  async function startRouting() {
    const saved = groups.find((group) => group.id === draft.id);
    if (!saved || JSON.stringify(saved) !== JSON.stringify(draft)) {
      toast.error("请先保存当前配置");
      return;
    }
    const defaultWorker = draft.workers.find(
      (worker) => worker.id === draft.defaultWorker,
    );
    if (!defaultWorker) {
      toast.error("请选择默认工作模型");
      return;
    }
    await run(async () => {
      await proxyApi.setProxyTakeoverForApp(
        draft.appType,
        true,
        false,
        defaultWorker.providerId,
      );
      toast.success(
        "本地路由已启动，请在客户端选择下方的路由组模型；Codex 模型目录变化后需重启客户端",
      );
    });
  }

  return (
    <>
      <AppPageHeader icon={<Route className="h-5 w-5" />} title="意图路由" />
      <div
        id="main-content"
        className="min-h-0 flex-1 overflow-y-auto px-6 pb-10 pt-4"
      >
        <p className="mb-4 text-sm text-muted-foreground">
          一个判断模型选择当前阶段的工作模型，连续请求复用决定。判断调用也会产生费用。
        </p>
        <div className="grid items-start gap-6 lg:grid-cols-[210px_1fr]">
          <aside className="grid gap-2">
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => {
                setDraft(createRouteGroup(draft.appType));
                setSimulation(null);
              }}
            >
              <Plus className="mr-2 h-4 w-4" />
              新建路由组
            </Button>
            {groups.map((group) => (
              <button
                key={group.id}
                className={`rounded-md border px-3 py-2 text-left ${group.id === draft.id ? "border-primary bg-primary/5" : "border-border"}`}
                onClick={() => {
                  setDraft(structuredClone(group));
                  setSimulation(null);
                }}
              >
                <span className="block font-medium">{group.name}</span>
                <span className="text-xs text-muted-foreground">
                  {group.appType} · {group.enabled ? "自动路由" : "已暂停"}
                </span>
              </button>
            ))}
          </aside>
          <section className="grid gap-5">
            <div className="grid gap-3 rounded-lg border p-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="grid gap-1 text-sm">
                  名称
                  <Input
                    aria-label="路由组名称"
                    value={draft.name}
                    onChange={(event) =>
                      setDraft({ ...draft, name: event.target.value })
                    }
                  />
                </label>
                <label className="grid gap-1 text-sm">
                  客户端
                  <select
                    aria-label="客户端"
                    className={selectStyle}
                    value={draft.appType}
                    onChange={(event) => {
                      setDraft(
                        createRouteGroup(
                          event.target.value as RouteGroup["appType"],
                        ),
                      );
                      setSimulation(null);
                    }}
                  >
                    <option value="codex">Codex</option>
                    <option value="claude">Claude Code</option>
                  </select>
                </label>
              </div>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={draft.enabled}
                  onChange={(event) =>
                    setDraft({ ...draft, enabled: event.target.checked })
                  }
                />
                启用自动路由（暂停后使用默认模型）
              </label>
              <div className="grid gap-3 sm:grid-cols-2">
                <ProviderSelect
                  label="判断模型供应商"
                  value={draft.judge.providerId}
                  providers={providers}
                  onChange={(providerId) =>
                    setDraft({
                      ...draft,
                      judge: { ...draft.judge, providerId },
                    })
                  }
                />
                <ModelPicker
                  appType={draft.appType}
                  providerId={draft.judge.providerId}
                  label="判断模型名称"
                  value={draft.judge.model}
                  onChange={(model) =>
                    setDraft({ ...draft, judge: { ...draft.judge, model } })
                  }
                />
              </div>
            </div>
            <div className="grid gap-3">
              <div className="flex items-center justify-between">
                <h2 className="font-medium">工作模型</h2>
                <Button variant="outline" size="sm" onClick={addWorker}>
                  <Plus className="mr-1 h-4 w-4" />
                  添加工作模型
                </Button>
              </div>
              {draft.workers.length === 0 && (
                <p className="text-sm text-muted-foreground">
                  至少添加一个工作模型；多个工作模型可按能力和成本分流。
                </p>
              )}
              {draft.workers.map((worker, index) => (
                <div
                  key={worker.id}
                  className="grid gap-3 rounded-lg border p-4"
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium">
                      工作模型 {index + 1}
                    </span>
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={`删除工作模型 ${index + 1}`}
                      onClick={() =>
                        setDraft({
                          ...draft,
                          workers: draft.workers.filter(
                            (entry) => entry.id !== worker.id,
                          ),
                        })
                      }
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <ProviderSelect
                      label={`工作模型 ${index + 1} 供应商`}
                      value={worker.providerId}
                      providers={providers}
                      onChange={(providerId) =>
                        changeWorker(worker.id, { providerId })
                      }
                    />
                    <ModelPicker
                      appType={draft.appType}
                      providerId={worker.providerId}
                      label={`工作模型 ${index + 1} 名称`}
                      value={worker.model}
                      showCapabilities
                      onChange={(model, capabilities) =>
                        changeWorker(worker.id, { model, ...capabilities })
                      }
                    />
                  </div>
                  <label className="grid gap-1 text-sm">
                    擅长任务与成本说明
                    <textarea
                      aria-label={`工作模型 ${index + 1} 能力说明`}
                      className="min-h-16 rounded-md border border-input bg-background p-2 text-sm"
                      placeholder="例如：便宜且快速，适合简单查询与机械修改；复杂排错交给另一个模型"
                      value={worker.description}
                      onChange={(event) =>
                        changeWorker(worker.id, {
                          description: event.target.value,
                        })
                      }
                    />
                  </label>
                  <div className="flex flex-wrap items-end gap-4">
                    <label className="grid gap-1 text-sm">
                      上下文容量
                      <Input
                        className="w-36"
                        type="number"
                        min={1024}
                        aria-label={`工作模型 ${index + 1} 上下文容量`}
                        value={worker.contextTokens}
                        onChange={(event) =>
                          changeWorker(worker.id, {
                            contextTokens: Number(event.target.value),
                          })
                        }
                      />
                    </label>
                    <label className="flex gap-2 py-2 text-sm">
                      <input
                        type="checkbox"
                        checked={worker.tools}
                        onChange={(event) =>
                          changeWorker(worker.id, {
                            tools: event.target.checked,
                          })
                        }
                      />
                      工具调用
                    </label>
                    <label className="flex gap-2 py-2 text-sm">
                      <input
                        type="checkbox"
                        checked={worker.images}
                        onChange={(event) =>
                          changeWorker(worker.id, {
                            images: event.target.checked,
                          })
                        }
                      />
                      图片输入
                    </label>
                  </div>
                </div>
              ))}
            </div>
            <div className="grid gap-3 rounded-lg border p-4 sm:grid-cols-2">
              {(
                [
                  ["defaultWorker", "默认工作模型"],
                  ["fallbackWorker", "异常回退模型"],
                  ["pinnedWorker", "手动固定模型"],
                ] as const
              ).map(([key, label]) => (
                <label key={key} className="grid gap-1 text-sm">
                  {label}
                  <select
                    aria-label={label}
                    className={selectStyle}
                    value={draft[key] ?? ""}
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        [key]:
                          event.target.value ||
                          (key === "pinnedWorker" ? null : ""),
                      })
                    }
                  >
                    <option value="">
                      {key === "pinnedWorker" ? "自动选择" : "请选择"}
                    </option>
                    {draft.workers.map((worker, index) => (
                      <option key={worker.id} value={worker.id}>
                        {worker.model || `工作模型 ${index + 1}`}
                      </option>
                    ))}
                  </select>
                </label>
              ))}
              <label className="grid gap-1 text-sm">
                判断方式
                <select
                  aria-label="判断方式"
                  className={selectStyle}
                  value={draft.mode}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      mode: event.target.value as RouteGroup["mode"],
                    })
                  }
                >
                  <option value="stage">阶段判断：一轮任务内可切换</option>
                  <option value="turn">每轮判断：用户新消息到来时判断</option>
                  <option value="observe">
                    观察模式：记录建议，执行默认模型
                  </option>
                </select>
              </label>
              <label className="grid gap-1 text-sm">
                连续请求数上限
                <Input
                  aria-label="连续请求数上限"
                  type="number"
                  min={1}
                  max={20}
                  value={draft.maxHoldRequests}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      maxHoldRequests: Number(event.target.value),
                    })
                  }
                />
              </label>
              <label className="grid gap-1 text-sm">
                每个会话最多判断次数
                <span className="text-xs text-muted-foreground">达到上限后停止判断，使用回退工作模型；不代表账户余额耗尽。0 表示不调用判断模型。</span>
                <Input
                  aria-label="判断次数预算"
                  type="number"
                  min={0}
                  max={1000}
                  value={draft.maxJudgeCalls}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      maxJudgeCalls: Number(event.target.value),
                    })
                  }
                />
              </label>
              <label className="grid gap-1 text-sm">
                判断超时（毫秒）
                <Input
                  aria-label="判断超时"
                  type="number"
                  min={100}
                  max={30000}
                  value={draft.judgeTimeoutMs}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      judgeTimeoutMs: Number(event.target.value),
                    })
                  }
                />
              </label>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button disabled={busy} onClick={save}>
                保存路由组
              </Button>
              <Button variant="outline" disabled={busy} onClick={startRouting}>
                启动客户端路由
              </Button>
              <Button
                variant="outline"
                disabled={busy}
                onClick={() =>
                  run(async () => {
                    await proxyApi.setProxyTakeoverForApp(draft.appType, false);
                    toast.success("已恢复客户端直连");
                  })
                }
              >
                恢复直连
              </Button>
              {groups.some((group) => group.id === draft.id) && (
                <Button
                  variant="ghost"
                  disabled={busy}
                  onClick={() =>
                    run(async () => {
                      await intentApi.remove(draft.id);
                      await refresh();
                      setDraft(createRouteGroup(draft.appType));
                    })
                  }
                >
                  删除路由组
                </Button>
              )}
            </div>
            <div className="rounded-lg border bg-muted/30 p-4 text-sm">
              <p>客户端选择的模型 ID：</p>
              <code className="mt-1 block break-all">
                {virtualModelId(draft.appType, draft.id)}
              </code>
              <p className="mt-2 text-muted-foreground">
                保存后启动本地路由，在客户端 /model 中选择该组。Codex
                目录变化后需重启客户端。无法移交的加密或增量状态会保持原模型。
              </p>
            </div>
            <div className="grid gap-3 rounded-lg border p-4">
              <h2 className="font-medium">试判断</h2>
              <p className="text-sm text-muted-foreground">
                调用真实判断模型，会产生费用；只显示选择结果，不执行任务。
              </p>
              <textarea
                aria-label="试判断任务"
                className="min-h-20 rounded-md border border-input bg-background p-2 text-sm"
                value={simulationText}
                onChange={(event) => setSimulationText(event.target.value)}
                placeholder="例如：为这个模块设计并发控制并解释取舍"
              />
              <Button
                variant="outline"
                disabled={busy || !simulationText.trim()}
                onClick={() => {
                  const problem = validateRouteGroup(draft);
                  if (problem) {
                    toast.error(problem);
                    return;
                  }
                  run(async () => {
                    setSimulation(
                      await intentApi.simulate(draft, simulationText),
                    );
                    await refresh();
                  });
                }}
              >
                判断一次
              </Button>
              {simulation && (
                <p role="status" className="text-sm">
                  {simulation.model} · {simulation.phase} ·{" "}
                  {sourceNames[simulation.source] ?? simulation.source}
                  <br />
                  {simulation.reason}
                </p>
              )}
            </div>
          </section>
        </div>
        <section className="mt-8 grid gap-3">
          <div className="flex items-center justify-between">
            <h2 className="font-medium">最近路由决定</h2>
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => run(refresh)}
            >
              刷新记录
            </Button>
          </div>
          <p className="text-sm text-muted-foreground">
            判断调用计入用量统计；工作模型费用可在用量统计中查看。未知费用不代表免费。
          </p>
          <div className="overflow-x-auto rounded-lg border">
            <table className="w-full text-left text-sm">
              <thead className="bg-muted/40">
                <tr>
                  {[
                    "阶段",
                    "工作模型",
                    "来源",
                    "判断耗时",
                    "判断费用",
                    "判断 → 选择 → 执行",
                  ].map((label) => (
                    <th className="px-3 py-2 font-medium" key={label}>
                      {label}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {records.map((record) => (
                  <tr key={record.id} className="border-t">
                    <td className="px-3 py-2">{record.phase}</td>
                    <td className="px-3 py-2">{record.model}</td>
                    <td className="px-3 py-2">
                      {sourceNames[record.source] ?? record.source}
                    </td>
                    <td className="px-3 py-2">{record.judgeMillis} ms</td>
                    <td className="px-3 py-2">
                      {record.judgeCostUsd == null
                        ? "未知 / 未调用"
                        : `$${record.judgeCostUsd}`}
                    </td>
                    <td className="max-w-xl px-3 py-2"><RouteTrace record={record} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
            {records.length === 0 && (
              <p className="p-4 text-sm text-muted-foreground">尚无路由记录</p>
            )}
          </div>
        </section>
      </div>
    </>
  );
}
