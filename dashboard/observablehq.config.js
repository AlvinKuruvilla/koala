// Observable Framework configuration for the koala WPT conformance
// dashboard. See https://observablehq.com/framework/config for the full
// option reference.

export default {
  title: "koala WPT",
  root: "src",
  // Single-page dashboard — no sidebar/header nav.
  pages: [],
  sidebar: false,
  header: "",
  footer: "",
  theme: ["near-midnight", "wide"],
  style: "style.css",
  head: `<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap">`,
};
