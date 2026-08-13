const http = require("http");
const path = require("path");
const fs = require("fs");
const { spawn } = require("child_process");

const root = path.resolve(__dirname, "../..");
const nodeModules = process.env.NOVAHUB_NODE_MODULES;
if (!nodeModules) throw new Error("NOVAHUB_NODE_MODULES is required");
process.env.NODE_PATH = nodeModules;
require("module").Module._initPaths();

const mime = { ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".svg": "image/svg+xml", ".json": "application/json; charset=utf-8" };
const server = http.createServer((request, response) => {
  const url = new URL(request.url, "http://127.0.0.1");
  const pathname = decodeURIComponent(url.pathname === "/" ? "/prototype/index.html" : url.pathname);
  const resolved = path.resolve(root, `.${pathname}`);
  if (!resolved.startsWith(root)) { response.writeHead(403).end(); return; }
  const file = fs.existsSync(resolved) && fs.statSync(resolved).isDirectory() ? path.join(resolved, "index.html") : resolved;
  fs.readFile(file, (error, data) => {
    if (error) { response.writeHead(404).end("Not found"); return; }
    response.writeHead(200, { "Content-Type": mime[path.extname(file)] || "application/octet-stream" });
    response.end(data);
  });
});

server.listen(4173, "127.0.0.1", () => {
  const { chromium } = require("playwright");
  const { run } = require(path.join(__dirname, "prototype.spec.js"));
  const browserType = { launch: options => chromium.launch({ ...options, executablePath: process.env.NOVAHUB_BROWSER_EXECUTABLE }) };
  run({ browserType, baseURL: "http://127.0.0.1:4173", report: result => console.log(JSON.stringify(result)) })
    .then(() => server.close(() => process.exit(0)))
    .catch(error => { console.error(error); server.close(() => process.exit(1)); });
});
