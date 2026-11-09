#!/usr/bin/env node
// update-contributors.mjs — regenerate the agent table in CONTRIBUTORS.md.
//
// Reads merged pull requests from the GitHub REST API, groups them by the
// `Agent:` trailer in each description, and rewrites the region between the
// BEGIN/END markers.
//
// Usage:
//   GITHUB_TOKEN=... GITHUB_REPOSITORY=owner/name node scripts/update-contributors.mjs
//
// The script is idempotent: running it twice produces the same file.

import { readFileSync, writeFileSync } from "node:fs";

const apiUrl = process.env.GITHUB_API_URL ?? "https://api.github.com";
const token = process.env.GITHUB_TOKEN;
const repo = process.env.GITHUB_REPOSITORY;

if (!token || !repo) {
  console.error("update-contributors: GITHUB_TOKEN and GITHUB_REPOSITORY are required");
  process.exit(2);
}

const TARGET = "CONTRIBUTORS.md";
const BEGIN = "<!-- BEGIN GENERATED: contributors -->";
const END = "<!-- END GENERATED: contributors -->";
const WINDOW_DAYS = 365;
const MAX_PAGES = 5;

async function gh(path) {
  const response = await fetch(`${apiUrl}${path}`, {
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${token}`,
      "X-GitHub-Api-Version": "2022-11-28",
    },
  });
  if (!response.ok) {
    throw new Error(`${path} -> ${response.status}: ${(await response.text()).slice(0, 300)}`);
  }
  return response.json();
}

function parseAgent(body) {
  if (!body) return null;
  const match = body.match(/^\s*Agent:\s*([A-Za-z0-9._-]+)\s*$/m);
  return match ? match[1] : null;
}

function areasOf(pull) {
  return (pull.labels ?? [])
    .map((label) => label.name)
    .filter((name) => name.startsWith("area:"))
    .map((name) => name.slice("area:".length));
}

function shortDate(iso) {
  return iso ? iso.slice(0, 10) : "—";
}

function render(agents, recent, totals) {
  const lines = [];
  lines.push(
    `_Generated from ${totals.merged} merged pull requests in the last ${WINDOW_DAYS} days. ` +
      `Last updated ${new Date().toISOString().slice(0, 10)}._`,
    "",
  );

  if (agents.length === 0) {
    lines.push("_No merged agent contributions recorded yet._");
    return lines.join("\n");
  }

  lines.push(
    "| Agent | Merged | Areas | Last merged |",
    "| --- | --- | --- | --- |",
  );
  for (const agent of agents) {
    const areas = agent.areas.length > 0 ? agent.areas.join(", ") : "—";
    lines.push(`| \`${agent.name}\` | ${agent.count} | ${areas} | ${shortDate(agent.last)} |`);
  }

  if (recent.length > 0) {
    lines.push("", "### Recent merged contributions", "");
    for (const pull of recent) {
      lines.push(`- #${pull.number} ${pull.title} — \`${pull.agent}\``);
    }
  }

  return lines.join("\n");
}

async function main() {
  const cutoff = Date.now() - WINDOW_DAYS * 24 * 60 * 60 * 1000;

  const pulls = [];
  for (let page = 1; page <= MAX_PAGES; page += 1) {
    const batch = await gh(
      `/repos/${repo}/pulls?state=closed&sort=updated&direction=desc&per_page=100&page=${page}`,
    );
    pulls.push(...batch);
    if (batch.length < 100) break;
    const oldest = batch[batch.length - 1]?.updated_at;
    if (oldest && new Date(oldest).getTime() < cutoff) break;
  }

  const merged = pulls.filter(
    (pull) => pull.merged_at && new Date(pull.merged_at).getTime() >= cutoff,
  );

  const byAgent = new Map();
  const recent = [];

  for (const pull of merged) {
    const agent = parseAgent(pull.body);
    if (!agent) continue;

    const entry = byAgent.get(agent) ?? { name: agent, count: 0, areas: new Set(), last: null };
    entry.count += 1;
    for (const area of areasOf(pull)) entry.areas.add(area);
    if (!entry.last || pull.merged_at > entry.last) entry.last = pull.merged_at;
    byAgent.set(agent, entry);

    recent.push({ number: pull.number, title: pull.title, agent, merged_at: pull.merged_at });
  }

  const agents = [...byAgent.values()]
    .map((entry) => ({ ...entry, areas: [...entry.areas].sort() }))
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));

  recent.sort((a, b) => (a.merged_at < b.merged_at ? 1 : -1));

  const content = readFileSync(TARGET, "utf8");
  const start = content.indexOf(BEGIN);
  const end = content.indexOf(END);
  if (start === -1 || end === -1 || end < start) {
    console.error(`update-contributors: markers missing from ${TARGET}`);
    process.exit(1);
  }

  const block = `${BEGIN}\n\n${render(agents, recent.slice(0, 10), { merged: merged.length })}\n\n`;
  const next = content.slice(0, start) + block + content.slice(end);

  if (next === content) {
    console.log("update-contributors: no change");
    return;
  }

  writeFileSync(TARGET, next);
  console.log(`update-contributors: recorded ${agents.length} agent(s) from ${merged.length} merged pull requests`);
}

main().catch((error) => {
  console.error(`update-contributors: ${error.message}`);
  process.exit(1);
});
