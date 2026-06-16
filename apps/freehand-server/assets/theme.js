const storageKey = "freehand-webui-theme";

export function initializeThemeToggle(root = document) {
  const buttons = Array.from(root.querySelectorAll(".theme-button"));
  const storedTheme = window.localStorage.getItem(storageKey) || "light";

  function applyTheme(theme) {
    const normalized = theme === "dark" ? "dark" : "light";
    document.body.classList.remove("theme-light", "theme-dark");
    document.body.classList.add(`theme-${normalized}`);
    buttons.forEach((button) => {
      button.classList.toggle("is-active", button.dataset.theme === normalized);
    });
    window.localStorage.setItem(storageKey, normalized);
  }

  buttons.forEach((button) => {
    button.addEventListener("click", () => applyTheme(button.dataset.theme));
  });

  applyTheme(storedTheme);
}
