const assert = require("assert");

const boards = [
  ["C1", "command", "idle", "NovaHub 命令中心"], ["C2", "command", "results", "Command Center / Mixed Results"], ["C3", "command", "preview", "Command Center / Preview"], ["C4", "command", "actions", "Command Center / Action Panel"], ["C5", "command", "partial", "Command Center / Partial Error"],
  ["B1", "clipboard", "history", "Clipboard / History"], ["B2", "clipboard", "preview", "Clipboard / Image Preview"], ["B3", "clipboard", "sensitive", "Clipboard / Sensitive"], ["B4", "clipboard", "clear", "Clipboard / Clear Confirm"],
  ["F1", "files", "results", "File Search / Results"], ["F2", "files", "filters", "File Search / Filters"], ["F3", "files", "preview", "File Search / Preview"], ["F4", "files", "offline", "File Search / Index Offline"],
  ["Q1", "library", "list", "Quicklinks & Snippets / Library"], ["Q2", "library", "parameters", "Quicklinks & Snippets / Parameters"], ["Q3", "library", "edit", "Quicklinks & Snippets / Edit"],
  ["W1", "windows", "layouts", "Window Manager / Layout Picker"], ["W2", "windows", "displays", "Window Manager / Multi-display"], ["W3", "windows", "permission", "Window Manager / Permission"],
  ["D1", "developer", "json", "Developer Toolkit / JSON Success"], ["D2", "developer", "json-error", "Developer Toolkit / JSON Error"], ["D3", "developer", "encoding", "Developer Toolkit / Encoding"], ["D4", "developer", "color", "Developer Toolkit / Color"], ["D5", "developer", "qr", "Developer Toolkit / QR"],
  ["T1", "translation", "input", "Translation / Input"], ["T2", "translation", "result", "Translation / Result"], ["T3", "translation", "error", "Translation / Offline Error"],
  ["G1", "gallery", "discover", "Gallery / Discover"], ["G2", "gallery", "detail", "Gallery / Detail"], ["G3", "gallery", "permissions", "Gallery / Permission Diff"], ["G4", "gallery", "uninstall", "Gallery / Uninstall"],
  ["R1", "migration", "source", "Migration / Source"], ["R2", "migration", "scan", "Migration / Scan"], ["R3", "migration", "review", "Migration / Review"], ["R4", "migration", "result", "Migration / Result"]
];

async function expectVisible(locator, message) {
  assert.equal(await locator.isVisible(), true, message);
}

async function run({ browserType, baseURL, report = () => {} }) {
  const browser = await browserType.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
  page.on("pageerror", error => errors.push(error.message));

  for (const [id, view, state, label] of boards) {
    await page.goto(`${baseURL}/prototype/?view=${view}&state=${state}`);
    const board = page.locator("[data-v1-board]:visible");
    await expectVisible(board, `${id} board is visible`);
    assert.equal(await board.getAttribute("data-board"), id, `${id} board id`);
    assert.equal(await board.getAttribute("aria-label"), label, `${id} board label`);
  }

  for (const [view, state, selector] of [["launcher", "idle", "#launcherView"], ["launcher", "actions", "#actionPanel.open"], ["translator", "result", "#translateResult:not(.hidden)"], ["plugins", "uninstall", "#uninstallModal:not(.hidden)"]]) {
    await page.goto(`${baseURL}/prototype/?view=${view}&state=${state}`);
    await expectVisible(page.locator(selector), `legacy ${view}/${state}`);
  }

  await page.goto(`${baseURL}/prototype/?view=command&state=results`);
  const search = page.getByRole("textbox", { name: "搜索所有能力" });
  await search.focus();
  await page.keyboard.press("ArrowDown");
  assert.equal(await page.locator("[data-v1-result].is-selected").getAttribute("data-index"), "1");
  await page.keyboard.press("Control+K");
  await expectVisible(page.locator("[data-command-actions]"), "command actions");
  await page.keyboard.press("Escape");
  assert.equal(await page.locator("[data-command-actions]").isVisible(), false, "command actions close");

  await page.emulateMedia({ reducedMotion: "reduce", colorScheme: "dark" });
  await page.goto(`${baseURL}/prototype/?view=migration&state=review&theme=dark&scale=200`);
  assert.equal((await page.locator("body").getAttribute("class"))?.includes("dark"), true, "dark mode");
  const scaledBodyFont = await page.locator("[data-v1-board]:visible .v1-list-item strong").first().evaluate(node => parseFloat(getComputedStyle(node).fontSize));
  assert.equal(scaledBodyFont >= 24, true, "200% text scale is applied to body copy");
  const scaledListLayout = await page.locator("[data-v1-board]:visible .v1-list-item").first().evaluate(row => {
    const icon = row.querySelector(".v1-icon").getBoundingClientRect();
    const copy = row.children[1].getBoundingClientRect();
    return { iconRight: icon.right, copyLeft: copy.left };
  });
  assert.equal(scaledListLayout.copyLeft - scaledListLayout.iconRight >= 8, true, "200% list icons keep readable spacing from copy");
  const overflow = await page.locator("[data-v1-board]:visible").evaluate(root => [root, ...root.querySelectorAll("*")]
    .filter(node => node.scrollWidth > node.clientWidth + 1 || node.scrollHeight > node.clientHeight + 1)
    .map(node => ({ tag: node.tagName, className: String(node.className), sw: node.scrollWidth, cw: node.clientWidth, sh: node.scrollHeight, ch: node.clientHeight })));
  assert.deepEqual(overflow, [], "no internal overflow at 200%");
  report({ boards: boards.length, errors });
  await browser.close();
}

module.exports = { boards, run };
