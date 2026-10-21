# Acceptance walkthrough

Five steps. Each one is a claim the code makes; this is how to check it in a real window. Nothing in
this list has been run by an automated agent — there is no desktop session in the build environment,
so this page is the hand-off.

## Before you start

- `just desktop` builds the UI and launches the shell. First launch compiles; later launches are fast.
- Have one **text-layer** PDF and one **scanned** PDF ready.
- Have an API key for a provider you can spend a few cents on (MiMo, DeepSeek, OpenAI, or a local
  Ollama with a vision model).

## 1. Select, ask, verify

1. Open a text PDF.
2. Drag across a paragraph; the bar appears above it.
3. Click `加入引用`; a chip appears in the tray.
4. Ask “这段在讲什么？”.
5. The answer streams in; if it cites `p.N`, click it.

**Expected:** the page jumps to N and its frame flashes. The status line shows token counts.

**If it fails:** check the status line first — it carries the provider's own error text. A keychain
prompt on the first save is normal; deny it and saving reports the reason.

## 2. Ask about an image

1. Click `框选`, drag a box around a formula or a figure, release.
2. A thumbnail chip appears.
3. Ask “这是什么？”.

**Expected:** the answer describes what is in the box, not the page's text.

**If it fails:** the model may not accept images. Use `测试连接` in settings; a model without vision
rejects the request with the provider's message.

## 3. Ask about a range

1. Type `31-45` in the range field and press `加入`.
2. Ask “这一章讲了什么？”.

**Expected:** pages without text are labelled as such; the answer covers the range and cites pages
inside it.

**If it fails:** a range above 20 pages says so above the composer; it is not truncated silently.

## 4. Ask about the current page

1. Clear the tray (remove its chips).
2. With the reader on page 40, ask “这一页在讲什么？”.

**Expected:** the answer is about page 40, and the prompt shows `[第 40 页]`.

## 5. Stop mid-answer

1. Ask a question that produces a long answer.
2. While text is arriving, click `停止`.

**Expected:** the partial answer stays, the status says `已停止`. The next send starts clean.

## What to report

For any failure: which step, what you saw, and the text of the status line or error. If the window
does not open at all, the terminal output of `just desktop` is the most useful thing.
