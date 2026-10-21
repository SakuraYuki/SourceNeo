// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ToolSettingsDialog } from "./ExternalTools";
import { installDomMocks } from "./test/fixtures";
const api = vi.hoisted(() => ({
  toolSettings: vi.fn(),
  saveToolSettings: vi.fn(),
}));
vi.mock("./api", () => ({ api }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});
it("prevents edits being silently discarded while settings are being saved", async () => {
  installDomMocks();
  api.toolSettings.mockResolvedValue({ terminal: "", editor: "" });
  api.saveToolSettings.mockReturnValue(new Promise(() => {}));
  render(<ToolSettingsDialog onClose={vi.fn()} />);
  const editor = await screen.findByLabelText("编辑器可执行文件");
  fireEvent.change(editor, { target: { value: "C:/tools/editor.exe" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect((editor as HTMLInputElement).disabled).toBe(true);
  expect(
    (screen.getByLabelText("终端可执行文件") as HTMLInputElement).disabled,
  ).toBe(true);
});
