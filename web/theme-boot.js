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
    // The installed page's bar too, each meta's own colour kept for System
    // (theme.js's THEME_COLORS and `applyTheme`).
    var colors = document.querySelectorAll('meta[name="theme-color"]');
    for (var i = 0; i < colors.length; i++) {
      if (!colors[i].dataset.scheme) colors[i].dataset.scheme = colors[i].getAttribute("content") || "";
      colors[i].setAttribute("content", theme === "dark" ? "#272724" : "#f1efe9");
    }
  }
})();
