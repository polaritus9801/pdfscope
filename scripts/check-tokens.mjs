// Fails when a component rule hard-codes a pixel value without saying why.
//
// Tokens live in :root and are exempt. Everything else must either use a token or carry a
// `/* size */` comment explaining that the number is a dimension (an icon, a hairline, a box),
// not spacing that belongs on the scale.
import { readFileSync } from "node:fs";

const css = readFileSync("ui/public/styles.css", "utf8");
const lines = css.split("\n");

let inRoot = false;
const offenders = [];

for (const [index, line] of lines.entries()) {
  if (line.trim().startsWith(":root")) inRoot = true;
  if (inRoot) {
    if (line.trim() === "}") inRoot = false;
    continue;
  }
  if (!/\d+px/.test(line)) continue;
  if (line.includes("/* size */") || line.includes("var(")) continue;
  offenders.push(`${index + 1}: ${line.trim()}`);
}

if (offenders.length > 0) {
  console.error(`hard-coded px outside the scale (${offenders.length}):`);
  for (const line of offenders) console.error("  " + line);
  process.exit(1);
}

console.log("token check: ok");
