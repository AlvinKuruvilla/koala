// Observable Framework configuration for the koala WPT conformance
// dashboard. See https://observablehq.com/framework/config for the full
// option reference.

export default {
  title: "koala WPT conformance",
  root: "src",
  pages: [
    { name: "Overview", path: "/" },
    { name: "By area", path: "/areas" },
  ],
  theme: ["air", "wide"],
  cleanUrls: true,
};
