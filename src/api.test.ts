import { beforeEach, expect, it, vi } from "vitest";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke, isTauri: () => false }));
import { api } from "./api";
import { makeHistory } from "./test/fixtures";
beforeEach(() => invoke.mockReset());
it("transfers only the appended history page using a pinned snapshot", async () => {
  const history = makeHistory(200);
  invoke.mockResolvedValueOnce({
    ...history,
    snapshot: "s1",
    commits: history.commits.slice(0, 100),
  });
  const first = await api.history("/paging", 100);
  invoke.mockResolvedValueOnce({
    ...history,
    snapshot: "s1",
    commits: history.commits.slice(100),
  });
  const next = await api.history("/paging", 200);
  expect(invoke).toHaveBeenLastCalledWith(
    "repo_history_page",
    expect.objectContaining({ offset: 100, snapshot: "s1" }),
  );
  expect(next.commits).toHaveLength(200);
  expect(next.commits[0]).toBe(first.commits[0]);
});
it("recovers an expired snapshot instead of endlessly retrying it", async () => {
  const history = makeHistory(200);
  invoke.mockResolvedValueOnce({
    ...history,
    snapshot: "expired",
    commits: history.commits.slice(0, 100),
  });
  await api.history("/expiry", 100);
  invoke
    .mockRejectedValueOnce({ kind: "snapshot", message: "expired" })
    .mockResolvedValueOnce({
      ...history,
      snapshot: "fresh",
      commits: history.commits.slice(0, 100),
    })
    .mockResolvedValueOnce({
      ...history,
      snapshot: "fresh",
      commits: history.commits.slice(100),
    });
  const recovered = await api.history("/expiry", 200);
  expect(recovered.commits).toHaveLength(200);
  expect(invoke).toHaveBeenLastCalledWith(
    "repo_history_page",
    expect.objectContaining({ snapshot: "fresh", offset: 100 }),
  );
});
it("a late first page cannot replace a newer snapshot", async () => {
  const history = makeHistory(100);
  let resolve!: (value: unknown) => void;
  invoke.mockReturnValueOnce(
    new Promise((r) => {
      resolve = r;
    }),
  );
  const old = api.history("/race", 100);
  invoke.mockResolvedValueOnce({ ...history, snapshot: "new" });
  await api.history("/race", 100);
  resolve({ ...history, snapshot: "old" });
  await old;
  invoke.mockResolvedValueOnce({ ...history, snapshot: "new" });
  await api.history("/race", 200);
  expect(invoke).toHaveBeenLastCalledWith(
    "repo_history_page",
    expect.objectContaining({ snapshot: "new" }),
  );
});

it("refreshes a short repository from a new snapshot", async () => {
  invoke.mockResolvedValueOnce({
    ...makeHistory(3),
    snapshot: "old-short",
    hasMore: false,
  });
  await api.history("/short", 100);
  invoke.mockResolvedValueOnce({
    ...makeHistory(4),
    snapshot: "new-short",
    hasMore: false,
  });
  expect((await api.history("/short", 100)).commits).toHaveLength(4);
  expect(invoke).toHaveBeenLastCalledWith(
    "repo_history_page",
    expect.objectContaining({ snapshot: null, offset: 0 }),
  );
});
