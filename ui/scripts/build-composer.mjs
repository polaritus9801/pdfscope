// Builds the composer as a classic script so index.html can load it before bridge.js.
import { build } from "esbuild";

await build({
  entryPoints: ["composer-src/index.js"],
  bundle: true,
  format: "iife",
  target: ["safari16"],
  platform: "browser",
  outfile: "public/composer.js",
  banner: {
    js: "/* PDFScope composer — ProseMirror (MIT). Generated from composer-src/index.js; run `npm run build:composer` after editing. */\n",
  },
  sourcemap: false,
  minify: true,
  logLevel: "info",
});
