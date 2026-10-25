# Composer

The question field is a ProseMirror editor. `@` references are inline atom nodes, so there is no
document position inside a pill; selection, arrow keys and deletion are handled by the editor model
rather than contenteditable patches.

`index.js` is bundled by `../scripts/build-composer.mjs` into `../public/composer.js` (classic script,
IIFE, minified). The generated file is committed so the app can be built without npm dependencies.

```sh
cd ui
npm install
npm run build:composer
```

Dependencies: ProseMirror (`prosemirror-model`, `-state`, `-view`, `-keymap`, `-history`,
`-commands`) and esbuild, all MIT.
