// KaTeX typesetting for rendered answers.
//
// The Markdown renderer leaves `$…$` and `$$…$$` untouched; this module finds them after Leptos
// updates the transcript. The observer is disconnected while KaTeX mutates the DOM, or its own
// insertions would retrigger it.

const options = {
  delimiters: [
    { left: "$$", right: "$$", display: true },
    { left: "$", right: "$", display: false },
  ],
  throwOnError: false,
  errorColor: "#b23a4e",
};

const observer = new MutationObserver(() => {
  const transcript = document.getElementById("transcript");
  if (!transcript || !transcript.textContent.includes("$")) return;
  if (!window.renderMathInElement) return;

  observer.disconnect();
  try {
    window.renderMathInElement(transcript, options);
  } finally {
    observer.observe(document.body, { childList: true, subtree: true });
  }
});

observer.observe(document.body, { childList: true, subtree: true });
