import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ModelPicker } from "./ModelPicker";
import { intentApi } from "@/lib/intent";
vi.mock("@/lib/intent", () => ({ intentApi: { models: vi.fn() } }));

describe("route model discovery", () => {
  it("applies advertised capabilities on selection, and preserves existing settings until requested", async () => {
    const capabilities = { contextTokens: 272000, images: true };
    vi.mocked(intentApi.models).mockResolvedValue([
      { id: "vision-model", ownedBy: null, capabilities },
    ]);
    const selected = vi.fn();
    render(
      <ModelPicker
        appType="codex"
        providerId="capability-discovery"
        label="能力模型"
        value="vision-model"
        onChange={selected}
        showCapabilities
      />,
    );
    await screen.findByText(/272000 token/);
    expect(selected).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("应用模型能力"));
    expect(selected).toHaveBeenCalledWith("vision-model", capabilities);
    selected.mockClear();
    fireEvent.change(screen.getByLabelText("能力模型可用模型"), {
      target: { value: "vision-model" },
    });
    expect(selected).toHaveBeenCalledWith("vision-model", capabilities);
  });
  it("fetches models on provider selection and selects the actual ID", async () => {
    vi.mocked(intentApi.models).mockResolvedValue([
      { id: "gpt-6-astra", ownedBy: null },
    ]);
    const selected = vi.fn();
    render(
      <ModelPicker
        appType="codex"
        providerId="official-discovery"
        label="模型"
        value=""
        onChange={selected}
      />,
    );
    expect(
      await screen.findByRole("option", { name: "gpt-6-astra" }),
    ).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("模型可用模型"), {
      target: { value: "gpt-6-astra" },
    });
    expect(selected).toHaveBeenCalledWith("gpt-6-astra");
    expect(intentApi.models).toHaveBeenCalledWith(
      "codex",
      "official-discovery",
    );
  });
  it("shows a recoverable discovery failure without deleting the saved model", async () => {
    vi.mocked(intentApi.models).mockRejectedValue(
      new Error("Login unavailable"),
    );
    render(
      <ModelPicker
        appType="codex"
        providerId="failed-discovery"
        label="模型"
        value="saved-model"
        onChange={vi.fn()}
      />,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Login unavailable",
    );
    expect(screen.getByLabelText("模型")).toHaveValue("saved-model");
    expect(screen.getByText("刷新模型")).not.toBeDisabled();
  });
});
