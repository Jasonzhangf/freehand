const storageKey = "freehand-prototype-theme";

function applyTheme(theme) {
  const nextTheme = theme === "dark" ? "dark" : "light";
  document.body.classList.remove("theme-light", "theme-dark");
  document.body.classList.add(`theme-${nextTheme}`);
  document.querySelectorAll(".theme-button").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.theme === nextTheme);
  });
  window.localStorage.setItem(storageKey, nextTheme);
}

function bindThemeButtons() {
  document.querySelectorAll(".theme-button").forEach((button) => {
    button.addEventListener("click", () => applyTheme(button.dataset.theme));
  });
}

const storedTheme = window.localStorage.getItem(storageKey) || "light";
applyTheme(storedTheme);
bindThemeButtons();
