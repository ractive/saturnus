// The theme button: System, Light, Dark in turn, kept under the
// emulator's key (`saturnus.theme`, web/theme.js), so both pages agree.
(function () {
  var KEY = "saturnus.theme";
  var button = document.getElementById("theme");
  if (!button) return;
  var names = { system: "follows the system", light: "light", dark: "dark" };
  var next = { system: "light", light: "dark", dark: "system" };
  function current() {
    var t = document.documentElement.getAttribute("data-theme");
    return t === "light" || t === "dark" ? t : "system";
  }
  function show() {
    var label = "Theme: " + names[current()];
    button.setAttribute("aria-label", label);
    button.title = label;
  }
  button.addEventListener("click", function () {
    var t = next[current()];
    if (t === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", t);
    try {
      if (t === "system") localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, t);
    } catch (e) { /* storage blocked: the choice lasts for this page */ }
    show();
  });
  show();
})();
