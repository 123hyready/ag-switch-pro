import { useEffect, useId, useState } from "react";
import {
  intentApi,
  type DiscoveredModel,
  type ModelCapabilities,
} from "@/lib/intent";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

const requests = new Map<string, Promise<DiscoveredModel[]>>();

export function ModelPicker({
  appType,
  providerId,
  label,
  value,
  onChange,
  showCapabilities = false,
}: {
  appType: string;
  providerId: string;
  label: string;
  value: string;
  onChange: (model: string, capabilities?: ModelCapabilities) => void;
  showCapabilities?: boolean;
}) {
  const listId = useId();
  const [models, setModels] = useState<DiscoveredModel[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const key = `${appType}:${providerId}`;
  useEffect(() => {
    let active = true;
    setModels([]);
    setError("");
    if (!providerId) {
      setLoading(false);
      return;
    }
    setLoading(true);
    let request = requests.get(key);
    if (!request) {
      request = intentApi.models(appType, providerId);
      requests.set(key, request);
      request.catch(() => {
        if (requests.get(key) === request) requests.delete(key);
      });
    }
    request
      .then((result) => {
        if (active) {
          setModels(result);
          if (!result.length) setError("供应商未返回可用模型");
        }
      })
      .catch((reason) => {
        if (active) setError(String(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [appType, providerId, key, refresh]);
  const capabilities = models.find((model) => model.id === value)?.capabilities;
  function selectModel(id: string) {
    const advertised = models.find((model) => model.id === id)?.capabilities;
    if (advertised) onChange(id, advertised);
    else onChange(id);
  }
  return (
    <label className="grid gap-1 text-sm">
      {label}
      <Input
        aria-label={label}
        list={listId}
        value={value}
        placeholder={loading ? "正在获取模型…" : "选择或搜索可用模型"}
        onChange={(event) => selectModel(event.target.value)}
      />
      <datalist id={listId}>
        {models.map((model) => (
          <option key={model.id} value={model.id} />
        ))}
      </datalist>
      <select
        aria-label={`${label}可用模型`}
        className="h-9 w-full rounded-md border border-input bg-background px-3"
        disabled={!models.length || loading}
        value={models.some((model) => model.id === value) ? value : ""}
        onChange={(event) => {
          if (event.target.value) selectModel(event.target.value);
        }}
      >
        <option value="">从自动获取的列表中选择</option>
        {models.map((model) => (
          <option key={model.id} value={model.id}>
            {model.id}
          </option>
        ))}
      </select>
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <span>
          {loading ? "正在自动获取" : `已获取 ${models.length} 个模型`}
        </span>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={!providerId || loading}
          onClick={() => {
            requests.delete(key);
            setRefresh((n) => n + 1);
          }}
        >
          刷新模型
        </Button>
      </div>
      {showCapabilities && capabilities && (
        <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <span>
            供应商能力：
            {capabilities.contextTokens != null
              ? `${capabilities.contextTokens} token`
              : "上下文未知"}
            {capabilities.images != null &&
              (capabilities.images ? " · 支持图片" : " · 仅文本")}
            （选择模型时自动填写）
          </span>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={() => selectModel(value)}
          >
            应用模型能力
          </Button>
        </div>
      )}
      {error && (
        <span role="alert" className="text-xs text-destructive">
          获取失败：{error}
        </span>
      )}
    </label>
  );
}
