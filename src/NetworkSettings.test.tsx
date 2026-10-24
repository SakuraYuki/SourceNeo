// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { NetworkSettingsDialog } from "./NetworkSettings";
import { installDomMocks } from "./test/fixtures";
const api = vi.hoisted(() => ({
  networkSettings: vi.fn(),
  saveNetworkSettings: vi.fn(),
}));
vi.mock("./api", () => ({ api }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

it("keeps invalid settings for correction and only closes after a successful save", async () => {
  installDomMocks();
  api.networkSettings.mockResolvedValue({ proxy: "", bypass: "" });
  api.saveNetworkSettings
    .mockRejectedValueOnce({ message: "代理地址无效" })
    .mockResolvedValueOnce(undefined);
  const close = vi.fn();
  render(<NetworkSettingsDialog onClose={close} />);
  const input = await screen.findByLabelText("代理地址");
  fireEvent.click(screen.getByRole("button", { name: /手动代理/ }));
  fireEvent.change(input, { target: { value: "invalid" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByRole("alert");
  expect(close).not.toHaveBeenCalled();
  expect((input as HTMLInputElement).value).toBe("invalid");
  fireEvent.change(input, { target: { value: "http://127.0.0.1:12345" } });
  fireEvent.change(screen.getByLabelText("直连域名"), {
    target: { value: ".devops.aliyuncs.com" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
  expect(api.saveNetworkSettings).toHaveBeenLastCalledWith({
    proxy: "http://127.0.0.1:12345",
    bypass: ".devops.aliyuncs.com",
  });
});

it("prevents dismissal and duplicate writes while saving", async () => {
  installDomMocks();
  api.networkSettings.mockResolvedValue({ proxy: "", bypass: "" });
  api.saveNetworkSettings.mockReturnValue(new Promise(() => {}));
  const close = vi.fn();
  render(<NetworkSettingsDialog onClose={close} />);
  const input = await screen.findByLabelText("代理地址");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect((input as HTMLInputElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  const cancel = new Event("cancel", { bubbles: true, cancelable: true });
  fireEvent(screen.getByRole("dialog"), cancel);
  expect(cancel.defaultPrevented).toBe(true);
  expect(close).not.toHaveBeenCalled();
  expect(api.saveNetworkSettings).toHaveBeenCalledOnce();
});

it("adds domain chips without submitting, deduplicates pasted domains, and removes individual rules", async () => {
  installDomMocks();
  api.networkSettings.mockResolvedValue({
    proxy: "http://127.0.0.1:12345",
    bypass: ".devops.aliyuncs.com",
  });
  render(<NetworkSettingsDialog onClose={vi.fn()} />);
  const input = await screen.findByLabelText("直连域名");
  fireEvent.change(input, {
    target: { value: "EXAMPLE.com, example.com localhost" },
  });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(api.saveNetworkSettings).not.toHaveBeenCalled();
  expect(
    screen.getAllByRole("button", { name: "移除直连域名 example.com" }),
  ).toHaveLength(1);
  expect(
    screen.getByRole("button", { name: "移除直连域名 localhost" }),
  ).toBeTruthy();
  fireEvent.click(
    screen.getByRole("button", { name: "移除直连域名 example.com" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(api.saveNetworkSettings).toHaveBeenCalledWith({
      proxy: "http://127.0.0.1:12345",
      bypass: ".devops.aliyuncs.com,localhost",
    }),
  );
});

it("includes an unfinished domain entry when saving and offers a Codeup shortcut", async () => {
  installDomMocks();
  api.networkSettings.mockResolvedValue({
    proxy: "http://127.0.0.1:12345",
    bypass: "",
  });
  render(<NetworkSettingsDialog onClose={vi.fn()} />);
  const input = await screen.findByLabelText("直连域名");
  fireEvent.click(screen.getByRole("button", { name: "Codeup 直连" }));
  expect(
    screen.getByRole("button", { name: "移除直连域名 .devops.aliyuncs.com" }),
  ).toBeTruthy();
  fireEvent.change(input, { target: { value: "example.com" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(api.saveNetworkSettings).toHaveBeenCalledWith({
      proxy: "http://127.0.0.1:12345",
      bypass: ".devops.aliyuncs.com,example.com",
    }),
  );
});

it("retains manual values while switching modes and clears only the proxy when saving inherited mode", async () => {
  installDomMocks();
  api.networkSettings.mockResolvedValue({
    proxy: "http://127.0.0.1:12345",
    bypass: ".devops.aliyuncs.com",
  });
  render(<NetworkSettingsDialog onClose={vi.fn()} />);
  const input = (await screen.findByLabelText("代理地址")) as HTMLInputElement;
  fireEvent.click(screen.getByRole("button", { name: /沿用 Git 配置/ }));
  expect(input.disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: /手动代理/ }));
  expect(input.value).toBe("http://127.0.0.1:12345");
  expect(input.disabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: /沿用 Git 配置/ }));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(api.saveNetworkSettings).toHaveBeenCalledWith({
      proxy: "",
      bypass: ".devops.aliyuncs.com",
    }),
  );
});
