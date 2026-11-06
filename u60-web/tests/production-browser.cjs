// Exercise the actual minified bundle. Development modules alone do not cover
// Rollup's CommonJS conversion of the shared firmware parsers.
const http = require("node:http");
const fs = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");
const root = path.resolve(__dirname, "../dist");
const server = http.createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  const relative =
    url.pathname === "/console/"
      ? "index.html"
      : url.pathname.replace(/^\/console\//, "");
  const file = path.resolve(root, relative);
  if (
    !file.startsWith(root + path.sep) ||
    !fs.existsSync(file) ||
    !fs.statSync(file).isFile()
  ) {
    res.writeHead(404);
    res.end();
    return;
  }
  const mime = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".css": "text/css",
    ".woff2": "font/woff2",
  };
  res.setHeader(
    "Content-Type",
    mime[path.extname(file)] || "application/octet-stream",
  );
  fs.createReadStream(file).pipe(res);
});
server.listen(0, "127.0.0.1", () => {
  const child = spawn(process.execPath, [path.join(__dirname, "browser.cjs")], {
    stdio: "inherit",
    env: {
      ...process.env,
      U60_WEB_TEST_URL: `http://127.0.0.1:${server.address().port}/console/`,
    },
  });
  child.on("error", (error) => {
    console.error(error);
    process.exitCode = 1;
    server.close();
  });
  child.on("exit", (code) => {
    process.exitCode = code || 0;
    server.close();
  });
});
