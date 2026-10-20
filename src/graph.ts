import type { Commit } from "./types";
export const palette = [
  "var(--graph-0)",
  "var(--graph-1)",
  "var(--graph-2)",
  "var(--graph-3)",
  "var(--graph-4)",
  "var(--graph-5)",
  "var(--graph-6)",
];
interface Lane {
  hash: string;
  color: number;
}
export interface Segment {
  from: number;
  to: number;
  color: number;
  type: "pass" | "in" | "out";
}
export interface GraphRow {
  commit: Commit;
  lane: number;
  color: number;
  segments: Segment[];
  width: number;
}
/** A lane is a pending ancestor, never a branch name. Every outgoing edge follows an actual parent hash. */
export function layoutGraph(commits: Commit[]): GraphRow[] {
  let lanes: Lane[] = [];
  let nextColor = 0;
  return commits.map((commit) => {
    let index = lanes.findIndex((l) => l.hash === commit.hash);
    const hadIncoming = index >= 0;
    if (index < 0) {
      index = lanes.findIndex((l) => !l.hash);
      if (index < 0) index = lanes.length;
      lanes[index] = { hash: commit.hash, color: nextColor++ % palette.length };
    }
    const old = lanes.map((l) => ({ ...l }));
    const color = old[index].color;
    lanes[index] = { hash: "", color };
    commit.parents.forEach((hash, i) => {
      if (lanes.some((l) => l.hash === hash)) return;
      let slot =
        i === 0 && lanes[index].hash === ""
          ? index
          : lanes.findIndex((l) => !l.hash);
      if (slot < 0) slot = lanes.length;
      lanes[slot] = {
        hash,
        color: i === 0 ? color : nextColor++ % palette.length,
      };
    });
    while (lanes.length && !lanes[lanes.length - 1].hash) lanes.pop();
    const segments: Segment[] = [];
    old.forEach((lane, i) => {
      if (i === index || !lane.hash) return;
      const target = lanes.findIndex((l) => l.hash === lane.hash);
      if (target >= 0)
        segments.push({ from: i, to: target, color: lane.color, type: "pass" });
    });
    if (hadIncoming)
      segments.push({ from: index, to: index, color, type: "in" });
    commit.parents.forEach((hash) => {
      const target = lanes.findIndex((l) => l.hash === hash);
      if (target >= 0)
        segments.push({
          from: index,
          to: target,
          color: lanes[target].color,
          type: "out",
        });
    });
    return {
      commit,
      lane: index,
      color,
      segments,
      width: Math.max(old.length, lanes.length, 1),
    };
  });
}
export function segmentPath(s: Segment, rowHeight = 40): string {
  const x = (n: number) => 16 + n * 18;
  const mid = rowHeight / 2;
  if (s.type === "in") return `M${x(s.from)},0 L${x(s.to)},${mid}`;
  if (s.type === "out")
    return `M${x(s.from)},${mid} C${x(s.from)},${rowHeight - 4} ${x(s.to)},${mid + 4} ${x(s.to)},${rowHeight}`;
  return `M${x(s.from)},0 C${x(s.from)},${mid} ${x(s.to)},${mid} ${x(s.to)},${rowHeight}`;
}
