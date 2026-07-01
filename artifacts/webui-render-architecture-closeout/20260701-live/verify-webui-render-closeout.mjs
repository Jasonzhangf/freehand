import { chromium } from "/opt/homebrew/lib/node_modules/playwright/index.mjs";
import { writeFile } from "node:fs/promises";

const sessionId = "cli-adp-sample-failure-1782903206134830000";
const outDir = "artifacts/webui-render-architecture-closeout/20260701-live";
const url = "http://127.0.0.1:4041";
const livePrompt =
  "Use the bash tool exactly once with command `sleep 8 && pwd`. After the tool result returns, complete with the Freehand completion schema and summarize the cwd. Do not answer before using the tool.";

function summarize() {
  const blocks = Array.from(document.querySelectorAll(".execution-block")).map((block, index) => ({
    index,
    turnId: block.dataset.turnId || "",
    live: block.dataset.live === "true",
    state: block.querySelector(".block-state")?.textContent || "",
    rows: Array.from(block.querySelectorAll(".execution-row")).map((row) => ({
      kind: Array.from(row.classList).find((name) => name.startsWith("execution-row-")) || "",
      turnId: row.dataset.turnId || "",
      toolCallId: row.dataset.toolCallId || "",
      status: row.querySelector(".execution-row-status")?.textContent || "",
      text: row.textContent?.replace(/\s+/g, " ").trim() || "",
    })),
  }));
  return {
    title: document.title,
    selected: window.localStorage.getItem("freehand-webui-selected-session"),
    commandStatus: document.querySelector("#command-status")?.textContent || "",
    turnStatus: document.querySelector("#turn-status")?.textContent || "",
    blockCount: blocks.length,
    liveCount: blocks.filter((block) => block.live).length,
    nonLastLiveCount: blocks.slice(0, -1).filter((block) => block.live).length,
    blocks,
  };
}

async function writeSummary(page, name) {
  const data = await page.evaluate(summarize);
  await writeFile(
    `${outDir}/${name}.json`,
    JSON.stringify(data, null, 2),
  );
  console.log(`${name}: blocks=${data.blockCount} live=${data.liveCount} nonLastLive=${data.nonLastLiveCount} status=${data.turnStatus}`);
  return data;
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1800 } });
  await context.addInitScript((selected) => {
    window.localStorage.setItem("freehand-webui-selected-session", selected);
  }, sessionId);
  const page = await context.newPage();
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await page.waitForSelector("#message-list", { timeout: 20000 });
  await page.waitForFunction(() => document.querySelectorAll(".execution-block").length >= 2, null, { timeout: 60000 });
  await page.screenshot({ path: `${outDir}/01-static-two-round-session.png`, fullPage: true });
  await writeSummary(page, "01-static-two-round-session");

  await page.fill("#composer-input", livePrompt);
  await page.click("#send-button");
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${outDir}/02-after-submit-immediate.png`, fullPage: true });
  await writeSummary(page, "02-after-submit-immediate");

  await page.waitForFunction(() => {
    const live = document.querySelectorAll(".execution-block[data-live='true']").length;
    const status = document.querySelector("#turn-status")?.textContent || "";
    return live >= 1 || /dispatching|thinking|tool running|tool executing/i.test(status);
  }, null, { timeout: 90000 }).catch(() => {});
  await page.waitForTimeout(1200);
  await page.screenshot({ path: `${outDir}/03-current-live-old-static.png`, fullPage: true });
  await writeSummary(page, "03-current-live-old-static");

  await page.waitForFunction(() => {
    const live = document.querySelectorAll(".execution-block[data-live='true']").length;
    const status = document.querySelector("#turn-status")?.textContent || "";
    return live === 0 && /completed|failed|blocked|cancelled|turn completed/i.test(status);
  }, null, { timeout: 240000 }).catch(() => {});
  await page.screenshot({ path: `${outDir}/04-terminal-no-stale-animation.png`, fullPage: true });
  await writeSummary(page, "04-terminal-no-stale-animation");

  await browser.close();
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
