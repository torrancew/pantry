class ThemeSwitcher {
  _theme = "auto";

  constructor() {
    this.theme = (window.localStorage?.getItem("pantryTheme") ?? this._theme);
    const buttons = document.querySelectorAll("a[data-theme-switcher]");
    buttons.forEach((button) => {
      button.addEventListener("click",
        (event) => {
          event.preventDefault();
          this.theme = button.getAttribute("data-theme-switcher");
          document.querySelector("details.dropdown")?.removeAttribute("open");
        },
        false
      );
    });
  }

  set theme(scheme) {
    if (scheme == "auto") {
      this._theme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    } else {
      this._theme = scheme;
    }
    document.querySelector("html")?.setAttribute("data-theme", this.theme);
    window.localStorage?.setItem("pantryTheme", this.theme);
  }

  get theme() {
    return this._theme;
  }
}

const switcher = new ThemeSwitcher();

// vim: set ts=4 sts=4 sw=4 et
