import { describe, it, expect } from "vitest";
import { layoutGraph } from "./graph";
import type { Commit } from "./types";
const c = (hash: string, parents: string[]): Commit => ({
  hash,
  parents,
  subject: hash,
  author: "test",
  date: "",
  refs: [],
});
describe("Git DAG layout", () => {
  it("routes both merge parents to their actual commit nodes", () => {
    const commits = [
      c("merge", ["local", "remote"]),
      c("local", ["base"]),
      c("remote", ["base"]),
      c("base", []),
    ];
    const rows = layoutGraph(commits);
    expect(rows[0].segments.filter((s) => s.type === "out")).toHaveLength(2);
    const pending = new Map<number, string>();
    rows.forEach((row) => {
      if (pending.has(row.lane))
        expect(pending.get(row.lane)).toBe(row.commit.hash);
      const next = new Map<number, string>();
      row.segments
        .filter((s) => s.type === "pass")
        .forEach((s) => next.set(s.to, pending.get(s.from)!));
      row.segments
        .filter((s) => s.type === "out")
        .forEach((s, i) => next.set(s.to, row.commit.parents[i]));
      pending.clear();
      next.forEach((v, k) => pending.set(k, v));
    });
    expect(pending.size).toBe(0);
  });
  it("keeps disconnected roots and octopus merges intact", () => {
    const rows = layoutGraph([
      c("m", ["a", "b", "c"]),
      c("a", ["root"]),
      c("b", ["root"]),
      c("c", ["root"]),
      c("root", []),
      c("other", []),
    ]);
    expect(rows[0].segments.filter((s) => s.type === "out")).toHaveLength(3);
    expect(rows[5].segments).toHaveLength(0);
  });
  it("leaves actual parent continuations at a pagination boundary", () => {
    const row = layoutGraph([c("tip", ["not-loaded"])])[0];
    expect(row.segments).toEqual([{ from: 0, to: 0, color: 0, type: "out" }]);
  });
});

it("preserves every pending ancestor in a dense DAG with many merges", () => {
  let seed = 73;
  const random = () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed;
  };
  const commits = Array.from({ length: 150 }, (_, i) => {
    if (i === 149) return c(`c${i}`, []);
    const parents = new Set([`c${i + 1}`]);
    if (random() % 3 === 0) parents.add(`c${i + 1 + (random() % (149 - i))}`);
    return c(`c${i}`, [...parents]);
  });
  let pending = new Map<number, string>();
  for (const row of layoutGraph(commits)) {
    if (pending.has(row.lane))
      expect(pending.get(row.lane)).toBe(row.commit.hash);
    const next = new Map<number, string>();
    for (const edge of row.segments.filter((s) => s.type === "pass")) {
      expect(pending.has(edge.from)).toBe(true);
      next.set(edge.to, pending.get(edge.from)!);
    }
    row.segments
      .filter((s) => s.type === "out")
      .forEach((edge, index) => {
        const parent = row.commit.parents[index];
        if (next.has(edge.to)) expect(next.get(edge.to)).toBe(parent);
        next.set(edge.to, parent);
      });
    for (const [lane, hash] of pending) {
      if (lane !== row.lane) expect([...next.values()]).toContain(hash);
    }
    pending = next;
  }
  expect(pending.size).toBe(0);
});

it("incremental layout matches a full layout across page boundaries", async () => {
  const { createGraphLayout } = await import("./graph");
  const { makeHistory } = await import("./test/fixtures");
  const commits = makeHistory(220).commits;
  commits[95].parents = [commits[96].hash, commits[110].hash];
  const append = createGraphLayout();
  expect([
    ...append(commits.slice(0, 100)),
    ...append(commits.slice(100)),
  ]).toEqual(layoutGraph(commits));
});
