#!/usr/bin/env node
// guard-pr.mjs — enforce the frozen-path and pull-request-shape rules.
//
// Runs from the *base* branch checkout, never from the pull request head, and
// reads everything it needs from the GitHub REST API. A contributor therefore
// cannot weaken the guard by editing this file in their branch.
//
// Modes:
//   --frozen       only the frozen-path check (hard gate)
//   --description  only the description/size checks (hard on size, soft on prose)
//
// Required environment:
//   GITHUB_TOKEN, GITHUB_REPOSITORY, GITHUB_EVENT_PATH

import { readFileSync } from "node:fs";

const apiUrl = process.env.GITHUB_API_URL ?? "https://api.github.com";
const token = process.env.GITHUB_TOKEN;
const repo = process.env.GITHUB_REPOSITORY;
const eventPath = process.env.GITHUB_EVENT_PATH;

if (!token || !repo || !eventPath) {
  console.error("guard-pr: GITHUB_TOKEN, GITHUB_REPOSITORY and GITHUB_EVENT_PATH are required");
  process.exit(2);
}

const mode = process.argv.includes("--frozen")
  ? "frozen"
  : process.argv.includes("--description")
    ? "description"
    : "all";

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const SECTIONS = ["What", "Why", "Change", "Verification", "Investigation", "Scope", "Compliance"];

const LIMITS = {
  S: 100,
  M: 400,
  L: 800, // warning band
  // above L: hard failure unless the size-xl-approved label is present
};

const MAX_FILES = 8;
const MAX_CRATES = 1;
const MAX_DESCRIPTION_CHARS = 2500;
const MAX_WHAT_CHARS = 140;
const MAX_WHY_BULLETS = 3;
const MAX_CHANGE_BULLETS = 5;

// Paths that inflate a diff without adding review burden.
const SIZE_EXEMPT = [
  /^Cargo\.lock$/,
  /^assets\/tailwind\.css$/,
  /^assets\/vendor\//,
  /^assets\/pricing\/btc_usd\.bin$/,
  /^public\/worker\//,
  /\.snap$/,
];

const OVERRIDE_LABEL = "maintainer-override";
const XL_APPROVED_LABEL = "size-xl-approved";
const MARKER = "<!-- agent-pr-guard -->";

const INVESTIGATION_FIELDS = [
  "Files read",
  "Pattern reused",
  "Alternatives rejected",
  "Prior art",
  "Deliberately untouched",
];

// ---------------------------------------------------------------------------
// GitHub helpers
// ---------------------------------------------------------------------------

async function gh(path, { method = "GET", body, retries = 1 } = {}) {
  for (let attempt = 0; ; attempt += 1) {
    const response = await fetch(`${apiUrl}${path}`, {
      method,
      headers: {
        Accept: "application/vnd.github+json",
        Authorization: `Bearer ${token}`,
        "X-GitHub-Api-Version": "2022-11-28",
        ...(body ? { "Content-Type": "application/json" } : {}),
      },
      body: body ? JSON.stringify(body) : undefined,
    });

    if (response.ok) {
      return response.status === 204 ? null : response.json();
    }

    const detail = await response.text();
    if (attempt < retries && response.status >= 500) continue;
    throw new Error(`${method} ${path} -> ${response.status}: ${detail.slice(0, 300)}`);
  }
}

async function listChangedFiles(number) {
  const files = [];
  for (let page = 1; page <= 5; page += 1) {
    const batch = await gh(`/repos/${repo}/pulls/${number}/files?per_page=100&page=${page}`);
    files.push(...batch);
    if (batch.length < 100) break;
  }
  return files;
}

// ---------------------------------------------------------------------------
// Frozen paths
// ---------------------------------------------------------------------------

function globToRegExp(glob) {
  const escaped = glob.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*");
  return new RegExp(`^${escaped}$`);
}

function loadFrozenPatterns() {
  let raw = "";
  try {
    raw = readFileSync(".github/frozen-paths.txt", "utf8");
  } catch {
    return null;
  }
  return raw
    .split(/\r?\n/)
    .map((line) => line.split("#")[0].trim())
    .filter(Boolean)
    .map(globToRegExp);
}

function checkFrozen(files, patterns) {
  const hits = files.filter((file) => patterns.some((re) => re.test(file.filename)));
  return hits.map((file) => file.filename);
}

// ---------------------------------------------------------------------------
// Description parsing
// ---------------------------------------------------------------------------

function stripHtmlComments(body) {
  return body.replace(/<!--[\s\S]*?-->/g, "");
}

function headings(body) {
  return [...body.matchAll(/^##\s+(.+?)\s*$/gm)].map((match) => match[1].trim());
}

function sections(body) {
  const map = new Map();
  let current = null;
  for (const line of body.split(/\r?\n/)) {
    const heading = line.match(/^##\s+(.+?)\s*$/);
    if (heading) {
      current = heading[1].trim().toLowerCase();
      map.set(current, []);
      continue;
    }
    if (current) map.get(current).push(line);
  }
  return map;
}

function sectionText(map, name) {
  return (map.get(name.toLowerCase()) ?? []).join("\n").trim();
}

function countBullets(text) {
  return text.split(/\r?\n/).filter((line) => /^\s*[-*]\s+\S/.test(line)).length;
}

function countChecked(text) {
  return (text.match(/^\s*[-*]\s+\[x\]/gim) ?? []).length;
}

function validateDescription(pr, body, changes) {
  const problems = [];
  const notes = [];

  const cleaned = stripHtmlComments(body).trim();
  if (!cleaned) {
    return { problems: ["description is empty; use the pull request template"], notes };
  }

  if (cleaned.length > MAX_DESCRIPTION_CHARS) {
    problems.push(
      `description is ${cleaned.length} characters; the limit is ${MAX_DESCRIPTION_CHARS}. Cut it to the required sections.`,
    );
  }

  const found = headings(cleaned).map((h) => h.toLowerCase());
  for (const required of SECTIONS) {
    if (!found.includes(required.toLowerCase())) {
      problems.push(`missing section \`## ${required}\``);
    }
  }

  const inOrder = found.filter((h) => SECTIONS.some((s) => s.toLowerCase() === h));
  const expected = SECTIONS.map((s) => s.toLowerCase()).filter((s) => inOrder.includes(s));
  if (inOrder.join("|") !== expected.join("|")) {
    problems.push(`sections are out of order; expected ${SECTIONS.map((s) => `## ${s}`).join(" → ")}`);
  }

  const map = sections(cleaned);

  const what = sectionText(map, "What").replace(/\s+/g, " ");
  if (!what) {
    problems.push("`## What` is empty");
  } else {
    if (what.length > MAX_WHAT_CHARS) {
      problems.push(
        `\`## What\` is ${what.length} characters; the limit is ${MAX_WHAT_CHARS}. State the change in one sentence.`,
      );
    }
    const sentences = what.split(/(?<=[.!?])\s+(?=[A-Z])/).filter(Boolean);
    if (sentences.length > 1) {
      problems.push("`## What` must be a single sentence");
    }
    if (/\band\b/i.test(what) && /\band\b.*\band\b/i.test(what)) {
      notes.push("`## What` contains more than one \"and\" — check that this is one logical change.");
    }
  }

  const why = sectionText(map, "Why");
  if (countBullets(why) > MAX_WHY_BULLETS) {
    problems.push(`\`## Why\` has more than ${MAX_WHY_BULLETS} bullets`);
  }

  const change = sectionText(map, "Change");
  if (countBullets(change) > MAX_CHANGE_BULLETS) {
    problems.push(`\`## Change\` has more than ${MAX_CHANGE_BULLETS} bullets`);
  }
  if (/^```/m.test(change)) {
    problems.push("`## Change` must not contain code blocks; describe the change, do not paste it");
  }

  if (!/\b(?:Fixes|Closes|Refs|Relates to)\s+#\d+/i.test(cleaned)) {
    problems.push("no issue reference; add `Fixes #N` or `Refs #N`");
  }

  const investigation = sectionText(map, "Investigation");
  for (const field of INVESTIGATION_FIELDS) {
    if (!new RegExp(`^\\s*[-*]?\\s*${field}\\s*:`, "im").test(investigation)) {
      problems.push(`\`## Investigation\` is missing the \`${field}:\` line`);
    }
  }

  const scope = sectionText(map, "Scope");
  const declared = scope.match(/Size:\s*(\d+)\s*lines?\s*\/\s*(\d+)\s*files?/i);
  if (!declared) {
    problems.push("`## Scope` must state `Size: <lines> lines / <files> files`");
  } else {
    if (Number(declared[1]) !== changes.lines) {
      problems.push(`declared ${declared[1]} lines but the diff has ${changes.lines} reviewable lines`);
    }
    if (Number(declared[2]) !== changes.files) {
      problems.push(`declared ${declared[2]} files but the diff touches ${changes.files} files`);
    }
  }

  const compliance = sectionText(map, "Compliance");
  if (countChecked(compliance) < 3) {
    problems.push("`## Compliance` must have all three checkboxes checked");
  }

  return { problems, notes };
}

// ---------------------------------------------------------------------------
// Size and shape
// ---------------------------------------------------------------------------

function measure(files) {
  const reviewable = files.filter(
    (file) => !SIZE_EXEMPT.some((re) => re.test(file.filename)),
  );
  const lines = reviewable.reduce((sum, file) => sum + file.additions + file.deletions, 0);
  const crates = new Set(
    files.map((file) => {
      const parts = file.filename.split("/");
      if (parts[0] === "crates" || parts[0] === "tools") return `${parts[0]}/${parts[1]}`;
      return "app";
    }),
  );
  return { lines, files: files.length, crates: [...crates] };
}

function validateShape(changes, labels) {
  const problems = [];
  const notes = [];

  if (changes.files > MAX_FILES) {
    problems.push(`touches ${changes.files} files; the limit is ${MAX_FILES}`);
  }
  if (changes.crates.length > MAX_CRATES) {
    problems.push(`touches ${changes.crates.length} workspace crates (${changes.crates.join(", ")}); the limit is ${MAX_CRATES}`);
  }
  if (changes.lines > LIMITS.L && !labels.includes(XL_APPROVED_LABEL)) {
    problems.push(
      `changes ${changes.lines} reviewable lines; anything above ${LIMITS.L} must be split (add the \`${XL_APPROVED_LABEL}\` label only if it genuinely cannot be)`,
    );
  } else if (changes.lines > LIMITS.M) {
    notes.push(`${changes.lines} reviewable lines is above the ${LIMITS.M}-line target; consider splitting`);
  }

  return { problems, notes };
}

// ---------------------------------------------------------------------------
// Duplicate work (advisory)
// ---------------------------------------------------------------------------

async function findPriorWork(number, body) {
  const refs = [...new Set([...body.matchAll(/#(\d+)/g)].map((match) => match[1]))].slice(0, 5);
  const found = [];
  for (const ref of refs) {
    try {
      const result = await gh(
        `/search/issues?q=${encodeURIComponent(`repo:${repo} is:pr is:merged "${ref}"`)}&per_page=5`,
      );
      for (const item of result.items ?? []) {
        if (item.number !== number) found.push(`#${item.number} ${item.title}`);
      }
    } catch {
      // Search is advisory; ignore rate limits and failures.
    }
  }
  return [...new Set(found)].slice(0, 5);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function render(frozenHits, shape, description, priorWork) {
  const lines = [MARKER, "### Contribution gate", ""];

  if (frozenHits.length > 0) {
    lines.push("**Frozen path modified.** These files are maintainer-owned:", "");
    for (const file of frozenHits) lines.push(`- \`${file}\``);
    lines.push("", "Revert them and open an issue with `needs-spec` instead.", "");
  }

  const problems = [...(shape?.problems ?? []), ...(description?.problems ?? [])];
  const notes = [...(shape?.notes ?? []), ...(description?.notes ?? [])];

  if (problems.length === 0) {
    lines.push("All automated shape and description checks passed.", "");
  } else {
    lines.push("Fix the following before review:", "");
    for (const problem of problems) lines.push(`- ${problem}`);
    lines.push("");
  }

  if (notes.length > 0) {
    lines.push("Advisory:", "");
    for (const note of notes) lines.push(`- ${note}`);
    lines.push("");
  }

  if (priorWork?.length) {
    lines.push("Prior merged work that mentions the same issue:", "");
    for (const item of priorWork) lines.push(`- ${item}`);
    lines.push("");
    lines.push("Confirm in `## Investigation → Prior art` why this is not a duplicate.", "");
  }

  return lines.join("\n");
}

async function upsertComment(number, body) {
  try {
    const comments = await gh(`/repos/${repo}/issues/${number}/comments?per_page=100`);
    const existing = comments.find((comment) => comment.body?.includes(MARKER));
    if (existing) {
      if (existing.body.trim() === body.trim()) return;
      await gh(`/repos/${repo}/issues/comments/${existing.id}`, { method: "PATCH", body: { body } });
      return;
    }
    await gh(`/repos/${repo}/issues/${number}/comments`, { method: "POST", body: { body } });
  } catch (error) {
    // Fork pull requests get a read-only token. The check status still carries
    // the verdict, so a missing comment is not fatal.
    console.log(`guard-pr: could not write the summary comment (${error.message})`);
  }
}

async function main() {
  const event = JSON.parse(readFileSync(eventPath, "utf8"));
  const pr = event.pull_request;
  if (!pr) {
    console.log("guard-pr: not a pull_request event; nothing to do");
    return;
  }

  const number = pr.number;
  const labels = (pr.labels ?? []).map((label) => label.name);
  const author = pr.user?.login ?? "";
  const body = pr.body ?? "";

  if (labels.includes(OVERRIDE_LABEL)) {
    console.log(`guard-pr: ${OVERRIDE_LABEL} label present; skipping`);
    return;
  }

  const files = await listChangedFiles(number);

  if (mode === "frozen") {
    const patterns = loadFrozenPatterns();
    if (!patterns) {
      console.error("guard-pr: .github/frozen-paths.txt missing from the base branch");
      process.exit(1);
    }
    if (author === repo.split("/")[0]) {
      console.log("guard-pr: maintainer-authored pull request; the frozen gate still applies");
    }

    const hits = checkFrozen(files, patterns);
    if (hits.length > 0) {
      await upsertComment(number, render(hits, null, null, null));
      console.error(`guard-pr: frozen paths modified: ${hits.join(", ")}`);
      process.exit(1);
    }
    console.log("guard-pr: no frozen path modified");
    return;
  }

  const changes = measure(files);
  const shape = validateShape(changes, labels);
  const description = validateDescription(pr, body, changes);
  const priorWork = await findPriorWork(number, body);
  const patterns = loadFrozenPatterns() ?? [];
  const frozenHits = checkFrozen(files, patterns);

  await upsertComment(number, render(frozenHits, shape, description, priorWork));

  const failures = [...shape.problems, ...description.problems];
  if (frozenHits.length > 0) failures.push(...frozenHits.map((file) => `frozen path: ${file}`));

  console.log(`guard-pr: ${changes.files} files, ${changes.lines} reviewable lines, crates: ${changes.crates.join(", ")}`);
  if (failures.length > 0) {
    for (const failure of failures) console.error(` - ${failure}`);
    process.exit(1);
  }
  console.log("guard-pr: pass");
}

main().catch((error) => {
  console.error(`guard-pr: ${error.message}`);
  process.exit(1);
});
