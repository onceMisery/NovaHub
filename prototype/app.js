const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

const recentResults = [
  { icon: "V", tone: "dark", title: "Visual Studio Code", sub: "最近打开 · NovaHub", type: "应用", key: "↵" },
  { icon: "文", tone: "translate-icon", title: "翻译文本", sub: "NovaHub 官方插件", type: "插件", key: "⌘⇧T" },
  { icon: "⌘", title: "打开插件管理", sub: "NovaHub 设置", type: "命令", key: "" },
  { icon: "N", title: "NovaHub architecture.md", sub: "C:\\user-data\\code\\github\\NovaHub\\docs", type: "文件", key: "" },
  { icon: "C", tone: "folder", title: "C:\\user-data\\code\\github", sub: "最近访问的文件夹", type: "文件夹", key: "" },
  { icon: "42", title: "计算器", sub: "输入算式立即计算", type: "内置", key: "" }
];

const searchResults = [
  { icon: "V", tone: "dark", title: "Visual Studio Code", sub: "Microsoft · 应用程序", type: "应用", key: "↵" },
  { icon: "C", title: "code.exe", sub: "C:\\Users\\cmira\\AppData\\Local\\Programs\\Microsoft VS Code", type: "文件", key: "" },
  { icon: "⌘", title: "在 Code 中打开当前文件夹", sub: "系统命令", type: "命令", key: "" },
  { icon: "N", title: "NovaHub source code", sub: "C:\\user-data\\code\\github\\NovaHub", type: "文件夹", key: "" },
  { icon: "</>", title: "Code Search", sub: "Developer Utilities · 插件", type: "插件", key: "" }
];

let selectedResult = 0;
let actionOpen = false;
let v1Selected = 0;
let v1ActionOpen = false;

function renderResults(items) {
  const list = $("#resultList");
  list.innerHTML = items.map((item, index) => `
    <div class="result-row ${index === selectedResult ? "selected" : ""}" role="option" aria-selected="${index === selectedResult}" data-index="${index}">
      <div class="result-icon ${item.tone || ""}">${item.icon}</div>
      <div class="result-copy"><strong>${item.title}</strong><span>${item.sub}</span></div>
      <div class="result-meta"><span class="result-type">${item.type}</span>${item.key ? `<kbd>${item.key}</kbd>` : ""}</div>
    </div>`).join("");
  $$(".result-row", list).forEach(row => row.addEventListener("mouseenter", () => {
    selectedResult = Number(row.dataset.index);
    renderResults(currentResults());
  }));
}

function currentResults() {
  return $("#launcherSearch").value.trim() ? searchResults : recentResults;
}

function updateLauncher() {
  const hasQuery = Boolean($("#launcherSearch").value.trim());
  $("#resultLabel").textContent = hasQuery ? "最佳匹配" : "最近使用";
  selectedResult = Math.min(selectedResult, currentResults().length - 1);
  renderResults(currentResults());
}

function toggleActions(next = !actionOpen) {
  actionOpen = next;
  $("#actionPanel").classList.toggle("open", actionOpen);
}

function showView(view) {
  $$(".stage").forEach(stage => stage.classList.toggle("hidden", stage.dataset.screen !== view));
  $$(".view-tab").forEach(tab => tab.classList.toggle("active", tab.dataset.view === view));
  if (view === "launcher") setTimeout(() => $("#launcherSearch").focus(), 0);
}

function showToast(id) {
  const toast = $(id);
  toast.classList.remove("hidden");
  clearTimeout(toast._timer);
  toast._timer = setTimeout(() => toast.classList.add("hidden"), 3200);
}

$$('.view-tab').forEach(tab => tab.addEventListener('click', () => showView(tab.dataset.view)));
$("#themeButton").addEventListener("click", () => document.body.classList.toggle("dark"));
$("#launcherSearch").addEventListener("input", updateLauncher);
$("#launcherSearch").addEventListener("keydown", event => {
  const count = currentResults().length;
  if (event.key === "ArrowDown") { event.preventDefault(); selectedResult = (selectedResult + 1) % count; renderResults(currentResults()); }
  if (event.key === "ArrowUp") { event.preventDefault(); selectedResult = (selectedResult - 1 + count) % count; renderResults(currentResults()); }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") { event.preventDefault(); toggleActions(true); }
  if (event.key === "Escape") { if (actionOpen) toggleActions(false); else { event.currentTarget.value = ""; updateLauncher(); } }
});
document.addEventListener("keydown", event => { if (event.key === "Escape" && actionOpen) toggleActions(false); });

function showTranslatorState(state) {
  $("#translatorForm").classList.toggle("hidden", state !== "form");
  $("#translateResult").classList.toggle("hidden", state !== "result");
  $("#pluginError").classList.toggle("hidden", state !== "error");
}

$("#clearTranslate").addEventListener("click", () => { $("#translateInput").value = ""; $("#translateInput").focus(); });
$("#translateButton").addEventListener("click", () => {
  if (!$("#translateInput").value.trim()) { showTranslatorState("error"); return; }
  $("#translatorForm").classList.add("loading");
  $("#translateButton").disabled = true;
  setTimeout(() => { $("#translatorForm").classList.remove("loading"); $("#translateButton").disabled = false; showTranslatorState("result"); }, 650);
});
$("#editTranslation").addEventListener("click", () => showTranslatorState("form"));
$("#errorBack").addEventListener("click", () => showTranslatorState("form"));
$("#retryTranslate").addEventListener("click", () => showTranslatorState("form"));
[$("#copyTranslation"), $("#copyTranslation2")].forEach(button => button.addEventListener("click", async () => {
  try { await navigator.clipboard.writeText("NovaHub keeps every search fast, quiet, and under your control."); } catch (_) { /* local file clipboard may be unavailable */ }
  showToast("#translationToast");
}));

const plugins = [
  { id: "translate", icon: "文", tone: "translate-icon", name: "翻译", sub: "2 个命令 · NovaHub Labs", version: "1.4.2" },
  { id: "json", icon: "{ }", name: "JSON 工具", sub: "3 个命令 · NovaHub Labs", version: "1.1.0" },
  { id: "color", icon: "#", tone: "folder", name: "颜色工具", sub: "2 个命令 · Lumen Studio", version: "0.9.4" }
];

function renderPlugins(items = plugins) {
  $("#pluginCount").textContent = `${items.length} 个插件`;
  $("#pluginList").innerHTML = items.map(plugin => `
    <button class="plugin-list-item" data-plugin="${plugin.id}">
      <div class="plugin-icon ${plugin.tone || ""}">${plugin.icon}</div>
      <div class="plugin-list-copy"><strong>${plugin.name}</strong><span>${plugin.sub}</span></div>
      <div class="plugin-list-meta"><span>v${plugin.version}</span><span class="status-dot"><i></i>已启用</span><svg viewBox="0 0 24 24"><path d="m9 18 6-6-6-6"></path></svg></div>
    </button>`).join("");
  $$(".plugin-list-item").forEach(row => row.addEventListener("click", () => {
    if (row.dataset.plugin === "translate") { $("#pluginListView").classList.add("hidden"); $("#pluginDetail").classList.remove("hidden"); }
  }));
}

$("#pluginSearch").addEventListener("input", event => {
  const query = event.target.value.trim().toLowerCase();
  renderPlugins(plugins.filter(plugin => plugin.name.toLowerCase().includes(query)));
});
$("#backToPlugins").addEventListener("click", () => { $("#pluginDetail").classList.add("hidden"); $("#pluginListView").classList.remove("hidden"); });
$("#pluginEnabled").addEventListener("change", event => { event.target.closest("label").querySelector("em").textContent = event.target.checked ? "已启用" : "已禁用"; });
$("#openUninstall").addEventListener("click", () => $("#uninstallModal").classList.remove("hidden"));
$("#cancelUninstall").addEventListener("click", () => $("#uninstallModal").classList.add("hidden"));
$("#confirmUninstall").addEventListener("click", () => {
  $("#uninstallModal").classList.add("hidden");
  $("#pluginDetail").classList.add("hidden");
  $("#pluginListView").classList.remove("hidden");
  plugins.splice(plugins.findIndex(plugin => plugin.id === "translate"), 1);
  renderPlugins();
  showToast("#uninstallToast");
});

renderResults(recentResults);
renderPlugins();

function applyPrototypeRoute() {
  const params = new URLSearchParams(window.location.search);
  const view = params.get("view") || "launcher";
  const state = params.get("state") || (view === "launcher" ? "idle" : view === "translator" ? "form" : "list");
  if (params.get("theme") === "dark") document.body.classList.add("dark");
  showView(["command", "clipboard", "files", "library", "windows", "developer", "translation", "gallery", "migration"].includes(view) ? "v1" : view);

  if (view === "launcher") {
    if (state === "results" || state === "actions") {
      $("#launcherSearch").value = params.get("query") || "code";
      updateLauncher();
    }
    if (state === "actions") toggleActions(true);
  }

  if (view === "translator") {
    if (state === "result") showTranslatorState("result");
    else if (state === "error") showTranslatorState("error");
    else showTranslatorState("form");
  }

  if (view === "plugins") {
    if (state === "detail" || state === "uninstall") {
      $("#pluginListView").classList.add("hidden");
      $("#pluginDetail").classList.remove("hidden");
    }
    if (state === "uninstall") $("#uninstallModal").classList.remove("hidden");
  }

  if (["command", "clipboard", "files", "library", "windows", "developer", "translation", "gallery", "migration"].includes(view)) {
    renderV1Board(view, state);
  }
}

const icon = name => ({
  search: '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7"></circle><path d="m20 20-3.5-3.5"></path></svg>',
  arrow: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6"></path></svg>',
  copy: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="8" y="8" width="12" height="12" rx="2"></rect><path d="M16 8V4H4v12h4"></path></svg>',
  info: '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"></circle><path d="M12 16v-4M12 8h.01"></path></svg>',
  trash: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6h18M8 6V4h8v2M8 10v7M12 10v7M16 10v7M5 6l1 15h12l1-15"></path></svg>',
  back: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m15 18-6-6 6-6"></path></svg>'
}[name] || "");

const v1BoardMeta = {
  command: { idle: ["C1", "NovaHub 命令中心", "最近使用与固定能力", "760px", "command"], results: ["C2", "Command Center / Mixed Results", "应用、文件、命令与插件统一搜索", "760px", "command"], preview: ["C3", "Command Center / Preview", "结果预览与上下文动作", "760px", "command"], actions: ["C4", "Command Center / Action Panel", "主动作、上下文动作与危险动作", "760px", "command"], partial: ["C5", "Command Center / Partial Error", "单个提供器失败不影响其他结果", "760px", "command"] },
  clipboard: { history: ["B1", "Clipboard / History", "文本、图片与文件历史", "840px", "clipboard"], preview: ["B2", "Clipboard / Image Preview", "受限图片预览与粘贴动作", "840px", "clipboard"], sensitive: ["B3", "Clipboard / Sensitive", "敏感内容默认不展示正文", "840px", "clipboard"], clear: ["B4", "Clipboard / Clear Confirm", "清理范围与不可恢复性", "840px", "clipboard"] },
  files: { results: ["F1", "File Search / Results", "系统索引结果与打开动作", "840px", "files"], filters: ["F2", "File Search / Filters", "类型、目录和修改时间筛选", "840px", "files"], preview: ["F3", "File Search / Preview", "文本预览与文件动作", "840px", "files"], offline: ["F4", "File Search / Index Offline", "索引不可用时的明确降级", "840px", "files"] },
  library: { list: ["Q1", "Quicklinks & Snippets / Library", "参数化链接与快捷短语", "840px", "library"], parameters: ["Q2", "Quicklinks & Snippets / Parameters", "执行前收集白名单变量", "840px", "library"], edit: ["Q3", "Quicklinks & Snippets / Edit", "编辑名称、变量与预览", "840px", "library"] },
  windows: { layouts: ["W1", "Window Manager / Layout Picker", "可视化布局与快捷键", "760px", "windows"], displays: ["W2", "Window Manager / Multi-display", "多显示器目标与窗口移动", "760px", "windows"], permission: ["W3", "Window Manager / Permission", "辅助功能授权说明", "760px", "windows"] },
  developer: { json: ["D1", "Developer Toolkit / JSON Success", "格式化、校验与复制", "880px", "developer"], "json-error": ["D2", "Developer Toolkit / JSON Error", "错误定位与恢复动作", "880px", "developer"], encoding: ["D3", "Developer Toolkit / Encoding", "Base64 与 URL 编解码", "880px", "developer"], color: ["D4", "Developer Toolkit / Color", "颜色格式与对比度", "880px", "developer"], qr: ["D5", "Developer Toolkit / QR", "二维码生成与保存", "880px", "developer"] },
  translation: { input: ["T1", "Translation / Input", "自动识别与双向翻译", "760px", "translation"], result: ["T2", "Translation / Result", "译文、原文与复制动作", "760px", "translation"], error: ["T3", "Translation / Offline Error", "离线与超时恢复", "760px", "translation"] },
  gallery: { discover: ["G1", "Gallery / Discover", "发现可信能力", "960px", "gallery"], detail: ["G2", "Gallery / Detail", "发布者、版本和用途", "960px", "gallery"], permissions: ["G3", "Gallery / Permission Diff", "安装前权限差异", "960px", "gallery"], uninstall: ["G4", "Gallery / Uninstall", "默认删除本地插件数据", "960px", "gallery"] },
  migration: { source: ["R1", "Migration / Source", "选择导出文件", "960px", "migration"], scan: ["R2", "Migration / Scan", "本地解析与分类", "960px", "migration"], review: ["R3", "Migration / Review", "冲突预览与逐项选择", "960px", "migration"], result: ["R4", "Migration / Result", "事务导入与迁移报告", "960px", "migration"] }
};

const v1Header = (title, sub, chip = "原型状态") => `<header class="v1-header"><div class="brand-mark small"><img src="../assets/brand/novahub-mark.svg" alt=""></div><div class="v1-title"><strong>${title}</strong><span>${sub}</span></div><span class="v1-chip"><i></i>${chip}</span></header>`;
const v1Footer = (hint = "↑↓ 选择") => `<footer class="v1-footer"><span><kbd>↑↓</kbd> ${hint}</span><span><kbd>↵</kbd> 执行</span><span><kbd>Esc</kbd> 返回</span><span class="v1-spacer">NovaHub 官方声明式视图</span></footer>`;
const resultRow = (item, index, selected = false) => `<button class="v1-result ${selected ? "is-selected" : ""}" data-v1-result data-index="${index}" role="option" aria-selected="${selected}"><span class="v1-result-icon ${item.tone || ""}">${item.icon}</span><span class="v1-result-copy"><strong>${item.title}</strong><span>${item.sub}</span></span><span class="v1-result-meta"><span class="v1-tag">${item.type}</span>${item.key ? `<kbd>${item.key}</kbd>` : ""}</span></button>`;

function commandBoard(state, meta) {
  const items = [
    { icon: "V", title: "Visual Studio Code", sub: "Microsoft · 应用程序", type: "应用", key: "↵" },
    { icon: "⌘", title: "打开插件管理", sub: "NovaHub 设置", type: "命令", key: "" },
    { icon: "N", title: "NovaHub source code", sub: "C:\\user-data\\code\\github\\NovaHub", type: "文件", key: "" },
    { icon: "文", tone: "green", title: "翻译文本", sub: "NovaHub 官方插件", type: "插件", key: "⌘⇧T" },
    { icon: "42", tone: "amber", title: "18 × 24 = 432", sub: "计算器 · 本地结果", type: "计算", key: "" }
  ];
  const selected = state === "idle" ? 0 : v1Selected;
  const partial = state === "partial";
  return `<div class="v1-window compact" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header("NovaHub 命令中心", meta[2], partial ? "文件搜索暂时不可用" : "5 个提供器就绪")}<div class="v1-body"><div class="v1-main"><label class="v1-search">${icon("search")}<input id="v1CommandSearch" aria-label="搜索所有能力" value="${state === "idle" ? "" : "code"}" placeholder="搜索应用、文件、命令和插件..."><kbd>⌘ Space</kbd></label><div class="v1-section-head"><span>${state === "idle" ? "最近使用" : "最佳匹配"}</span><button type="button">⌘P 筛选提供器</button></div><div class="v1-results" role="listbox">${items.map((item, index) => resultRow(item, index, index === selected)).join("")}</div>${partial ? `<div class="v1-callout warning" style="margin-top:12px">${icon("info")}<span>文件搜索提供器没有响应。其他提供器仍可使用。<button class="v1-button text">查看状态</button></span></div>` : ""}</div><aside class="v1-side"><div class="v1-section-head" style="margin-top:0"><span>快速预览</span><span class="v1-tag">${state === "idle" ? "最近打开" : "应用"}</span></div><div class="v1-preview"><h2>${state === "idle" ? "Visual Studio Code" : "Visual Studio Code"}</h2><p>打开编辑器并恢复上次工作区。</p><dl><dt>位置</dt><dd>Microsoft · 应用程序</dd><dt>上次使用</dt><dd>今天 10:42</dd><dt>主动作</dt><dd>打开</dd></dl></div><div class="v1-actions"><button class="v1-action selected">${icon("arrow")}<span>打开</span><kbd>↵</kbd></button><button class="v1-action">${icon("copy")}<span>复制名称</span></button><button class="v1-action">${icon("info")}<span>查看详情</span></button></div>${state === "actions" ? `<div data-command-actions class="v1-actions" style="margin-top:16px"><div class="v1-section-head" style="margin:0 0 3px"><span>操作面板</span><kbd>Esc</kbd></div><button class="v1-action selected">${icon("arrow")}<span>打开</span><kbd>↵</kbd></button><button class="v1-action">${icon("copy")}<span>复制路径</span></button><button class="v1-action danger">${icon("trash")}<span>移到废纸篓</span></button></div>` : ""}</aside></div>${v1Footer("↑↓ 选择")}</div>`;
}

function clipboardBoard(state, meta) {
  const items = [{ icon: "Aa", title: "NovaHub 让每一次查找都保持快速", sub: "来自 Visual Studio Code · 2 分钟前", type: "文本" }, { icon: "▧", tone: "image", title: "design-review.png", sub: "来自 Finder · 12 分钟前 · 1.8 MB", type: "图片" }, { icon: "▣", title: "architecture.md", sub: "来自 NovaHub · 昨天", type: "文件" }];
  const sensitive = state === "sensitive";
  return `<div class="v1-window" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header("剪贴板", meta[2], sensitive ? "敏感内容已保护" : "7 天 · 284 条")}<div class="v1-body"><div class="v1-main"><div class="v1-toolbar"><button class="v1-filter active">全部</button><button class="v1-filter">文本</button><button class="v1-filter">图片</button><button class="v1-filter">文件</button><span class="v1-spacer"></span><button class="v1-button">暂停记录</button></div><div class="v1-section-head"><span>${state === "clear" ? "清理范围" : "今天"}</span><span>⌘F 搜索</span></div>${state === "clear" ? `<div class="v1-confirm"><h2>清理剪贴板历史？</h2><p>选择要删除的本地内容。固定项目不会被默认删除。</p><ul><li>未固定的文本、图片和文件记录</li><li>加密载荷与缩略图缓存</li></ul><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">取消</button><button class="v1-button danger">清理未固定项</button></div></div>` : `<div class="v1-list">${items.map(item => `<div class="v1-list-item"><span class="v1-icon ${item.tone || ""}">${sensitive && item.type === "文本" ? "••" : item.icon}</span><div><strong>${sensitive && item.type === "文本" ? "敏感内容已隐藏" : item.title}</strong><span>${sensitive && item.type === "文本" ? "来源已排除预览 · 可粘贴" : item.sub}</span></div><span class="v1-tag">${item.type}</span></div>`).join("")}</div>`}</div><aside class="v1-side"><div class="v1-section-head" style="margin-top:0"><span>预览</span><span class="v1-tag">${state === "preview" ? "图片" : "本地"}</span></div>${state === "preview" ? `<div class="v1-preview"><div class="v1-icon image" style="width:100%;height:170px;margin-bottom:12px">▧</div><h2>design-review.png</h2><p>图片预览受尺寸预算限制。</p></div>` : `<div class="v1-callout">${icon("info")}<span>剪贴板内容只保存在本机，插件不能读取历史集合。</span></div>`}<div class="v1-actions"><button class="v1-action selected">${icon("arrow")}<span>粘贴到当前应用</span><kbd>↵</kbd></button><button class="v1-action">${icon("copy")}<span>复制为纯文本</span></button></div></aside></div>${v1Footer("↑↓ 选择历史项")}</div>`;
}

function filesBoard(state, meta) {
  const offline = state === "offline";
  const files = [{ icon: "N", title: "NovaHub architecture.md", sub: "C:\\user-data\\code\\github\\NovaHub\\docs · 12 KB", type: "MD" }, { icon: "J", title: "package.json", sub: "C:\\user-data\\code\\github\\NovaHub\\prototype · 3 KB", type: "JSON" }, { icon: "▧", title: "novahub-components.png", sub: "C:\\user-data\\code\\github\\NovaHub\\work · 248 KB", type: "PNG" }];
  return `<div class="v1-window" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header("文件搜索", meta[2], offline ? "系统索引不可用" : "Windows Search")}<div class="v1-body"><div class="v1-main"><label class="v1-search">${icon("search")}<input aria-label="搜索文件" value="${offline ? "" : "architecture"}" placeholder="搜索文件、目录和内容..."><kbd>⌘F</kbd></label><div class="v1-toolbar" style="margin-top:12px"><button class="v1-filter ${state === "filters" ? "active" : ""}">类型：全部</button><button class="v1-filter">目录：当前工作区</button><button class="v1-filter">修改时间</button></div>${offline ? `<div class="v1-empty" style="margin-top:18px"><div><strong>无法访问系统索引</strong><p>NovaHub 不会在后台执行全盘扫描。</p><button class="v1-button primary" style="margin-top:12px">打开索引设置</button></div></div>` : `<div class="v1-section-head"><span>3 个结果</span><span>按相关性排序</span></div><div class="v1-list">${files.map((file, index) => `<button class="v1-list-item" style="text-align:left;background:${index === 0 ? "var(--selected)" : "transparent"};border:0;border-bottom:1px solid var(--border);color:var(--text)"><span class="v1-icon">${file.icon}</span><div><strong>${file.title}</strong><span>${file.sub}</span></div><span class="v1-tag">${file.type}</span></button>`).join("")}</div>`}</div><aside class="v1-side"><div class="v1-section-head" style="margin-top:0"><span>快速预览</span></div>${state === "preview" ? `<div class="v1-preview"><h2>NovaHub architecture.md</h2><p>Rust + Slint + Wasmtime Component Model</p><dl><dt>大小</dt><dd>12 KB</dd><dt>修改</dt><dd>今天 09:48</dd><dt>路径</dt><dd>C:\\user-data\\code\\github\\NovaHub\\docs</dd></dl></div>` : `<div class="v1-callout">${icon("info")}<span>预览只读取系统索引返回的目标文件，打开操作交给系统。</span></div>`}<div class="v1-actions"><button class="v1-action selected">${icon("arrow")}<span>打开</span><kbd>↵</kbd></button><button class="v1-action">${icon("copy")}<span>复制路径</span></button></div></aside></div>${v1Footer("↑↓ 选择文件")}</div>`;
}

function libraryBoard(state, meta) {
  const edit = state === "edit";
  const parameters = state === "parameters";
  return `<div class="v1-window" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header("Quicklinks 与 Snippets", meta[2], "本地配置")}<div class="v1-body single"><div class="v1-main"><div class="v1-toolbar"><button class="v1-filter active">全部</button><button class="v1-filter">Quicklinks</button><button class="v1-filter">Snippets</button><span class="v1-spacer"></span><button class="v1-button primary">+ 新建</button></div>${edit ? `<div class="v1-form" style="margin-top:18px"><label class="v1-field">名称<input value="搜索 NovaHub 文档"><small>在命令中心中显示的名称</small></label><label class="v1-field">URL 模板<input value="https://www.google.com/search?q={query}"><small>变量只允许白名单字段，并在执行前预览编码结果</small></label><label class="v1-field">变量<textarea>{"query":""}</textarea></label><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">取消</button><button class="v1-button primary">保存 Quicklink</button></div></div>` : parameters ? `<div class="v1-form" style="margin-top:18px"><div class="v1-callout">${icon("info")}<span>执行前需要 1 个参数。NovaHub 不执行 URL 中的脚本或任意代码。</span></div><label class="v1-field">搜索关键词<input aria-label="搜索关键词" placeholder="例如：Rust Slint"></label><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">取消</button><button class="v1-button primary">打开链接</button></div></div>` : `<div class="v1-section-head"><span>6 个本地项目</span><span>最近使用</span></div><div class="v1-list"><div class="v1-list-item"><span class="v1-icon green">↗</span><div><strong>搜索 NovaHub 文档</strong><span>Quicklink · google.com · 需要 query 参数</span></div><button class="v1-button">执行</button></div><div class="v1-list-item"><span class="v1-icon">Aa</span><div><strong>会议纪要模板</strong><span>Snippet · 日期、剪贴板和光标变量</span></div><button class="v1-button">插入</button></div><div class="v1-list-item"><span class="v1-icon amber">→</span><div><strong>打开设计评审</strong><span>Quicklink · figma.com · 无参数</span></div><button class="v1-button">执行</button></div></div>`}</div></div>${v1Footer("↑↓ 选择项目")}</div>`;
}

function windowsBoard(state, meta) {
  const permission = state === "permission";
  const displays = state === "displays";
  return `<div class="v1-window compact" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header("窗口管理", meta[2], permission ? "需要辅助功能权限" : "2 个显示器")}<div class="v1-body single"><div class="v1-main">${permission ? `<div class="v1-callout warning">${icon("info")}<span>窗口管理需要系统辅助功能权限，用于读取窗口位置并应用布局。NovaHub 不会读取窗口内容。<button class="v1-button primary" style="margin:10px 0 0">打开系统设置</button></span></div>` : displays ? `<div class="v1-section-head"><span>选择目标显示器</span></div><div class="v1-list"><div class="v1-list-item" style="background:var(--selected)"><span class="v1-icon">1</span><div><strong>内置显示器</strong><span>2560 × 1440 · 当前活动显示器</span></div><span class="v1-tag">当前</span></div><div class="v1-list-item"><span class="v1-icon">2</span><div><strong>外接显示器</strong><span>1920 × 1080 · 左侧</span></div><span class="v1-tag">可用</span></div></div>` : `<div class="v1-section-head"><span>选择布局</span><span>当前窗口：Visual Studio Code</span></div><div class="v1-layout-grid"><button class="v1-layout-cell active">左半屏</button><button class="v1-layout-cell">右半屏</button><button class="v1-layout-cell">全屏</button><button class="v1-layout-cell">左上</button><button class="v1-layout-cell">右上</button><button class="v1-layout-cell">居中</button></div><div class="v1-callout" style="margin-top:18px">${icon("info")}<span>布局只影响当前窗口，不会改变其他应用的位置。</span></div>`}</div></div>${v1Footer("↑↓ 选择布局")}</div>`;
}

function developerBoard(state, meta) {
  const error = state === "json-error";
  const color = state === "color";
  const qr = state === "qr";
  const encoding = state === "encoding";
  const title = color ? "颜色工具" : qr ? "二维码工具" : encoding ? "编码工具" : "JSON 工具";
  const input = color ? "#1769E0" : qr ? "https://novahub.local/docs" : encoding ? "NovaHub" : '{\n  "name": "NovaHub",\n  "mode": "local-first"\n}';
  const output = error ? "第 3 行，第 12 列：缺少逗号" : color ? "HEX  #1769E0\nRGB  23, 105, 224\nHSL  216°, 81%, 48%\n\n对比度\n正文文字  4.8:1  AA" : qr ? "[二维码预览]\n\n可复制 PNG 或保存到文件" : encoding ? "Base64\nTm92YUh1Yg==\n\nURL\nNovaHub" : '{\n  "name": "NovaHub",\n  "mode": "local-first"\n}';
  return `<div class="v1-window tool" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header(title, meta[2], "官方 WASM 工具") }<div class="v1-body single"><div class="v1-main"><div class="v1-toolbar"><button class="v1-filter ${!color && !qr && !encoding ? "active" : ""}">JSON</button><button class="v1-filter ${encoding ? "active" : ""}">编码</button><button class="v1-filter ${color ? "active" : ""}">颜色</button><button class="v1-filter ${qr ? "active" : ""}">二维码</button><span class="v1-spacer"></span><span class="v1-tag">最大输入 2 MiB</span></div><div class="v1-tool-grid" style="margin-top:16px"><section class="v1-code-pane"><header><span>输入</span><button class="v1-button text">清空</button></header><textarea class="v1-field" aria-label="工具输入" style="border:0;border-radius:0;resize:none;padding:14px;min-height:0;font-family:Consolas,monospace;font-size:12px;line-height:1.6">${input}</textarea></section><section class="v1-code-pane ${error ? "error" : ""}"><header><span>${error ? "诊断" : "结果"}</span><button class="v1-button text">${error ? "查看位置" : "复制"}</button></header><pre>${output}</pre></section></div>${error ? `<div class="v1-callout danger" style="margin-top:12px">${icon("info")}<span>输入不是有效 JSON。修复错误后重试，结果不会覆盖原文。</span></div>` : `<div class="v1-callout" style="margin-top:12px">${icon("info")}<span>所有计算在本地 WASM 会话中完成；插件不能访问主窗口 DOM。</span></div>`}</div></div>${v1Footer("Tab 切换输入与结果")}</div>`;
}

function translationBoard(state, meta) {
  const result = state === "result";
  const error = state === "error";
  const content = error
    ? `<div class="v1-empty"><div><span class="v1-icon danger">!</span><strong>翻译服务暂时不可用</strong><p>请检查网络后重试。输入内容仍保留在本地。</p><div class="v1-toolbar" style="justify-content:center;margin-top:16px"><button class="v1-button">返回编辑</button><button class="v1-button primary">重试</button></div></div></div>`
    : result
      ? `<div class="v1-form"><div class="v1-section-head"><span>简体中文 → English</span><span class="v1-tag">translate.example.com</span></div><div class="v1-preview"><h2>NovaHub keeps every search fast, quiet, and under your control.</h2><div class="v1-section-head"><span>原文</span></div><p>NovaHub 让每一次查找都保持快速、安静和可控。</p></div><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">重新编辑</button><button class="v1-button primary">复制译文</button></div></div>`
      : `<div class="v1-form"><div class="v1-toolbar"><label class="v1-field" style="flex:1">源语言<select><option>自动检测</option><option>简体中文</option></select></label><button class="v1-button" aria-label="交换语言">⇄</button><label class="v1-field" style="flex:1">目标语言<select><option>English</option><option>简体中文</option></select></label></div><label class="v1-field">待翻译内容<textarea>NovaHub 让每一次查找都保持快速、安静和可控。</textarea><small>27 / 5,000 · 内容仅发送到已授权域名</small></label><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">清空</button><button class="v1-button primary">翻译</button></div></div>`;
  return `<div class="v1-window compact" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header("翻译", meta[2], error ? "网络不可用" : "官方 WASM 插件")}<div class="v1-body single"><div class="v1-main">${content}</div></div>${v1Footer(result ? "复制或重新编辑" : "Tab 切换字段")}</div>`;
}

function galleryBoard(state, meta) {
  const permission = state === "permissions";
  const uninstall = state === "uninstall";
  const detail = state === "detail";
  return `<div class="v1-window gallery" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header("扩展市场", meta[2], "签名与权限可见")}<div class="v1-body single"><div class="v1-main"><div class="v1-toolbar"><label class="v1-search" style="flex:1;height:38px">${icon("search")}<input aria-label="搜索扩展" placeholder="搜索翻译、JSON、颜色和更多能力..."></label><button class="v1-filter active">全部</button><button class="v1-filter">已安装</button></div>${uninstall ? `<div class="v1-confirm" style="margin-top:20px"><h2>卸载“翻译”？</h2><p>NovaHub 将删除插件代码、设置、缓存、凭据、权限和插件数据。</p><ul><li>外部文件不在删除范围</li><li>远端服务数据不在删除范围</li></ul><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">取消</button><button class="v1-button danger">卸载并删除数据</button></div></div>` : detail || permission ? `<div class="v1-preview" style="margin-top:20px"><div class="v1-toolbar"><span class="v1-icon green">文</span><div><h2 style="margin:0">翻译</h2><p>NovaHub Labs · v1.4.2 · 2 个命令</p></div><span class="v1-spacer"></span><span class="v1-tag">Rust/WASM</span></div><div class="v1-section-head"><span>用途</span></div><p>自动识别语言并翻译选中的文本，网络权限限定为 translate.example.com。</p>${permission ? `<div class="v1-callout warning" style="margin-top:14px">${icon("info")}<span>本次更新新增网络域名权限。安装前需要明确确认。</span></div>` : ""}<div class="v1-toolbar" style="margin-top:18px"><span class="v1-spacer"></span><button class="v1-button">查看变更日志</button><button class="v1-button primary">${detail ? "已安装" : "安装扩展"}</button></div></div>` : `<div class="v1-gallery-grid" style="margin-top:20px"><article class="v1-gallery-item"><span class="v1-icon green">文</span><div><h3>翻译</h3><p>自动识别语言，快速翻译选中文本。</p><span class="v1-tag">网络 · 已签名</span></div><button class="v1-button">详情</button></article><article class="v1-gallery-item"><span class="v1-icon">{ }</span><div><h3>JSON 工具</h3><p>格式化、校验、压缩与 JSONPath 查询。</p><span class="v1-tag">本地 · 官方</span></div><button class="v1-button">已安装</button></article><article class="v1-gallery-item"><span class="v1-icon amber">#</span><div><h3>颜色工具</h3><p>取色、颜色转换与 WCAG 对比度。</p><span class="v1-tag">取色 · 社区</span></div><button class="v1-button">安装</button></article></div>`}</div></div>${v1Footer("↑↓ 选择扩展")}</div>`;
}

function migrationBoard(state, meta) {
  const source = state === "source";
  const scan = state === "scan";
  const result = state === "result";
  return `<div class="v1-window migration" data-v1-board data-board="${meta[0]}" aria-label="${meta[1]}">${v1Header("迁移中心", meta[2], "本地处理 · 不执行源代码")}<div class="v1-body single"><div class="v1-main"><div class="v1-migration-steps"><span class="v1-step ${source ? "active" : ""}"><b>1</b>选择来源</span><i class="v1-step-line"></i><span class="v1-step ${scan ? "active" : ""}"><b>2</b>扫描分类</span><i class="v1-step-line"></i><span class="v1-step ${!source && !scan && !result ? "active" : ""}"><b>3</b>审阅导入</span><i class="v1-step-line"></i><span class="v1-step ${result ? "active" : ""}"><b>4</b>完成</span></div>${source ? `<div class="v1-form"><div class="v1-callout">${icon("info")}<span>NovaHub 只解析你选择的 uTools/Raycast 导出文件，不读取凭据、剪贴板历史或云端数据。</span></div><label class="v1-field">导出文件<input type="text" value="未选择文件" readonly><small>支持公开配置、Quicklinks、Snippets 和插件清单。</small></label><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">取消</button><button class="v1-button primary">选择导出文件</button></div></div>` : scan ? `<div class="v1-form"><div class="v1-callout success">${icon("info")}<span>已完成本地扫描。未执行任何脚本或插件代码。</span></div><div class="v1-list"><div class="v1-list-item"><span class="v1-icon green">✓</span><div><strong>可直接导入</strong><span>4 个 Quicklinks · 3 个 Snippets</span></div><span class="v1-tag">7 项</span></div><div class="v1-list-item"><span class="v1-icon amber">↔</span><div><strong>可映射到 NovaHub 能力</strong><span>2 个别名 · 1 个搜索引擎</span></div><span class="v1-tag">3 项</span></div><div class="v1-list-item"><span class="v1-icon">!</span><div><strong>需要重写为 WASM</strong><span>5 个插件命令 · 仅生成迁移报告</span></div><span class="v1-tag">5 项</span></div></div></div>` : result ? `<div class="v1-form"><div class="v1-callout success">${icon("info")}<span>迁移完成。7 项配置已写入本地，5 个插件需要重写。</span></div><div class="v1-preview"><h2>迁移报告</h2><dl><dt>来源</dt><dd>Raycast export.json</dd><dt>已导入</dt><dd>7 项 Quicklinks / Snippets</dd><dt>待重写</dt><dd>5 个 WASM 插件任务</dd><dt>报告</dt><dd>novahub-migration-20260813.json</dd></dl></div><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">导出报告</button><button class="v1-button primary">完成</button></div></div>` : `<div class="v1-form"><div class="v1-callout warning">${icon("info")}<span>导入前请检查冲突。目标项目不会执行源脚本。</span></div><div class="v1-list"><div class="v1-list-item"><span class="v1-icon green">↗</span><div><strong>搜索 NovaHub 文档</strong><span>Quicklink · 将覆盖同名项目</span></div><input type="checkbox" checked aria-label="导入搜索 NovaHub 文档"></div><div class="v1-list-item"><span class="v1-icon">Aa</span><div><strong>会议纪要模板</strong><span>Snippet · 新建项目</span></div><input type="checkbox" checked aria-label="导入会议纪要模板"></div></div><div class="v1-toolbar"><span class="v1-spacer"></span><button class="v1-button">返回扫描</button><button class="v1-button primary">确认导入</button></div></div>`}</div></div>${v1Footer("Tab 切换导入项")}</div>`;
}

function renderV1Board(view, state) {
  const meta = v1BoardMeta[view]?.[state] || v1BoardMeta[view]?.[Object.keys(v1BoardMeta[view] || {})[0]];
  if (!meta) return;
  const renderers = { command: commandBoard, clipboard: clipboardBoard, files: filesBoard, library: libraryBoard, windows: windowsBoard, developer: developerBoard, translation: translationBoard, gallery: galleryBoard, migration: migrationBoard };
  const renderer = renderers[view];
  if (!renderer) return;
  $("#v1BoardMount").innerHTML = renderer(state, meta);
  const board = $("[data-v1-board]");
  if (new URLSearchParams(window.location.search).get("scale") === "200") board.classList.add("text-scale-200");
  if (view === "command") {
    const search = $("#v1CommandSearch");
    const results = $$("[data-v1-result]");
    const rerenderCommand = nextState => {
      renderV1Board(view, nextState);
      $("#v1CommandSearch").focus();
    };
    results.forEach(row => row.addEventListener("mouseenter", () => { v1Selected = Number(row.dataset.index); renderV1Board(view, state); }));
    search.addEventListener("keydown", event => {
      if (event.key === "ArrowDown") { event.preventDefault(); v1Selected = (v1Selected + 1) % results.length; rerenderCommand(state); }
      if (event.key === "ArrowUp") { event.preventDefault(); v1Selected = (v1Selected - 1 + results.length) % results.length; rerenderCommand(state); }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") { event.preventDefault(); v1ActionOpen = true; rerenderCommand("actions"); }
      if (event.key === "Escape" && v1ActionOpen) { v1ActionOpen = false; rerenderCommand("results"); }
    });
  }
}

applyPrototypeRoute();
