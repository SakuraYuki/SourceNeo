// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { HistoryView } from "./HistoryView";
import {
  installDomMocks,
  makeHistory,
  ResizeObserverMock,
} from "./test/fixtures";

vi.mock("./api", () => ({ api: { copy: vi.fn() } }));
beforeEach(installDomMocks);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("history virtualization", () => {
  it("restores visible rows and observes the new container after a failed refresh", () => {
    const props = {
      history: makeHistory(300),
      loading: false,
      error: "",
      onMore: vi.fn(),
      onRetry: vi.fn(),
    };
    const view = render(<HistoryView {...props} />);
    const oldContainer = view.container.querySelector(".history-scroll")!;
    const oldObserver = ResizeObserverMock.instances[0];
    fireEvent.scroll(oldContainer, { target: { scrollTop: 3200 } });
    expect(screen.queryByText("Commit 0")).toBeNull();

    view.rerender(<HistoryView {...props} error="读取失败" />);
    fireEvent.click(screen.getByRole("button", { name: "重试加载" }));
    expect(props.onRetry).toHaveBeenCalledOnce();
    // The successful retry returns the same first commit and the same row count.
    view.rerender(<HistoryView {...props} />);
    const newContainer = view.container.querySelector(".history-scroll")!;
    expect(newContainer).not.toBe(oldContainer);
    expect(newContainer.scrollTop).toBe(0);
    expect(screen.getByText("Commit 0")).toBeTruthy();
    expect(oldObserver.disconnect).toHaveBeenCalledOnce();
    const newObserver = ResizeObserverMock.instances.at(-1)!;
    expect(newObserver).not.toBe(oldObserver);
    expect(newObserver.observe).toHaveBeenCalledWith(newContainer);
    act(() => newObserver.resize(960));
    expect(screen.getByText("Commit 30")).toBeTruthy();
  });

  it("keeps the scroll position when another page is appended", () => {
    const props = {
      history: makeHistory(200),
      loading: false,
      error: "",
      onMore: vi.fn(),
      onRetry: vi.fn(),
    };
    const view = render(<HistoryView {...props} />);
    const container = view.container.querySelector(".history-scroll")!;
    fireEvent.scroll(container, { target: { scrollTop: 3200 } });
    view.rerender(<HistoryView {...props} history={makeHistory(300)} />);
    expect(container.scrollTop).toBe(3200);
    expect(screen.getByText("Commit 100")).toBeTruthy();
    expect(screen.queryByText("Commit 0")).toBeNull();
  });
});
