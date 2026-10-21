// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { RepoMenu } from "./RepoMenu";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
function setup(popup = false) {
  const origin = document.createElement("button");
  if (popup) origin.setAttribute("aria-haspopup", "menu");
  document.body.append(origin);
  const close = vi.fn();
  const run = vi.fn();
  const view = render(
    <RepoMenu
      x={window.innerWidth - 2}
      y={window.innerHeight - 2}
      origin={origin}
      label="仓库操作"
      onClose={close}
      actions={[
        { label: "Fetch", run },
        { label: "同步", run, disabled: true },
        { label: "复制路径", run },
      ]}
    />,
  );
  return { origin, close, run, view };
}
it("keeps an anchored menu clear of its trigger in a short window", () => {
  vi.stubGlobal("innerHeight", 580);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
    function (this: HTMLElement) {
      return (
        this.tagName === "BUTTON"
          ? { top: 270, bottom: 300, width: 26, height: 30 }
          : { width: 224, height: 350 }
      ) as DOMRect;
    },
  );
  const { origin } = setup(true);
  const menu = screen.getByRole("menu");
  expect(menu.style.top).toBe("304px");
  expect(menu.style.maxHeight).toBe("268px");
  origin.remove();
});
it("clamps the menu and navigates enabled actions, restoring focus on Escape", () => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 224,
    height: 200,
  } as DOMRect);
  const { origin, close } = setup();
  const menu = screen.getByRole("menu");
  expect(menu.style.left).toBe(`${window.innerWidth - 232}px`);
  expect(menu.style.top).toBe(`${window.innerHeight - 208}px`);
  expect(document.activeElement).toBe(
    screen.getByRole("menuitem", { name: "Fetch" }),
  );
  fireEvent.keyDown(menu, { key: "ArrowDown" });
  expect(document.activeElement).toBe(
    screen.getByRole("menuitem", { name: "复制路径" }),
  );
  fireEvent.keyDown(menu, { key: "ArrowDown" });
  expect(document.activeElement).toBe(
    screen.getByRole("menuitem", { name: "Fetch" }),
  );
  fireEvent.keyDown(menu, { key: "End" });
  expect(document.activeElement).toBe(
    screen.getByRole("menuitem", { name: "复制路径" }),
  );
  fireEvent.keyDown(menu, { key: "Home" });
  expect(document.activeElement).toBe(
    screen.getByRole("menuitem", { name: "Fetch" }),
  );
  fireEvent.keyDown(menu, { key: "Escape" });
  expect(close).toHaveBeenCalled();
  expect(document.activeElement).toBe(origin);
  origin.remove();
});
it("dismisses outside, on resize and external scroll; an action closes before running", () => {
  const { close, run, origin, view } = setup();
  fireEvent.pointerDown(document.body);
  expect(close).toHaveBeenCalledTimes(1);
  fireEvent.resize(window);
  expect(close).toHaveBeenCalledTimes(2);
  fireEvent.scroll(document);
  expect(close).toHaveBeenCalledTimes(3);
  fireEvent.click(screen.getByRole("menuitem", { name: "Fetch" }));
  expect(run).toHaveBeenCalledTimes(1);
  expect(close.mock.invocationCallOrder[0]).toBeLessThan(
    run.mock.invocationCallOrder[0],
  );
  view.unmount();
  close.mockClear();
  fireEvent.resize(window);
  expect(close).not.toHaveBeenCalled();
  origin.remove();
});
