// The stored colour theme, applied before the first paint (a classic
// script in index.html's head, so no flash of the other theme); web/theme.js
// changes it later and says what the values mean.
(function () {
  var theme = null;
  try { theme = localStorage.getItem("saturnus.theme"); } catch (e) { /* storage blocked */ }
  if (theme === "light" || theme === "dark") {
    document.documentElement.setAttribute("data-theme", theme);
    var meta = document.querySelector('meta[name="color-scheme"]');
    if (meta) meta.setAttribute("content", theme);
  }
})();
