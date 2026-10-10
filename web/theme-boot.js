// The stored colour theme, applied before the first paint (a classic
// script in index.html's head, so no flash of the other theme); web/theme.js
// changes it later and says what the values mean. The logo's colours too
// (web/logo.js).
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
  // The remembered model's logo colours (web/logo.js's `LOGO_MODELS`),
  // so the headers' logo does not change colour once the page starts.
  var model = null;
  try { model = localStorage.getItem("saturnus.model"); } catch (e) { /* storage blocked */ }
  if (["48sx", "48gx", "49g", "38g", "39g", "40g", "42s"].indexOf(model) >= 0) document.documentElement.setAttribute("data-logo", model);
})();
