// Nostyra Online browser gate (classic script, runs before the app module; an inline
// script would be blocked by the CSP). The web edition needs the File System Access
// directory picker of desktop Chrome / Edge: other browsers (Firefox, Safari, Brave
// with its default settings) and phones / tablets get a static page instead, and the
// app module sees `data-unsupported-browser` on <html> and does not start.
(function () {
  "use strict";
  var ua = navigator.userAgent || "";
  var mobile = (navigator.userAgentData && navigator.userAgentData.mobile) || /Android|iPhone|iPad|iPod|Mobile/i.test(ua);
  if (typeof window.showDirectoryPicker === "function" && !mobile) return;

  document.documentElement.setAttribute("data-unsupported-browser", "");
  var base = document.currentScript ? document.currentScript.src.replace(/[^/]*$/, "") : "/online/";
  var style = document.createElement("link");
  style.rel = "stylesheet";
  style.href = base + "static-page.css";
  document.head.appendChild(style);

  function add(parent, tag, text, attrs) {
    var element = document.createElement(tag);
    if (text) element.textContent = text;
    for (var name in attrs || {}) element.setAttribute(name, attrs[name]);
    parent.appendChild(element);
    return element;
  }

  function render() {
    document.title = "Nostyra Online — use Chrome or Edge";
    document.body.textContent = "";
    var main = add(document.body, "main", "", { class: "static-page gate" });
    add(main, "p", "NOSTYRA", { class: "brand" });
    add(main, "h1", "Nostyra Online needs Chrome or Edge on a computer");
    add(
      main,
      "p",
      mobile
        ? "Phones and tablets cannot open your ROM folder from a web page. Open this page in Chrome or Microsoft Edge on a Windows, macOS or Linux computer."
        : "This browser cannot open folders on your computer, which Nostyra Online needs to read your ROMs and keep your saves. Open this page in Google Chrome or Microsoft Edge.",
    );
    if (navigator.brave) {
      add(main, "p", "Using Brave? Turn on brave://flags/#file-system-access-api, restart Brave and reload this page.");
    }
    var links = add(main, "p", "", { class: "links" });
    add(links, "a", "Get the Nostyra desktop app", { href: "https://nostyra.com" });
    add(links, "a", "Licences", { href: base + "licences.html" });
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", render);
  else render();
})();
