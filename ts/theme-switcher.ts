type Theme = "auto" | "dark" | "light";

class ThemeSwitcher {
  _theme: Theme = "auto";

  constructor() {
    this.theme = (<Theme>window.localStorage?.getItem("pantryTheme") ?? this._theme);
    const buttons = document.querySelectorAll("a[data-theme-switcher]");
    buttons.forEach((button) => {
      button.addEventListener(
        "click",
        (event) => {
          event.preventDefault();
          this.theme = <Theme>button.getAttribute("data-theme-switcher");
          document.querySelector("details.dropdown")?.removeAttribute("open");
        },
        false
      );
    });
  }

  set theme(scheme: Theme) {
    if (scheme == "auto") {
      this._theme = window.matchMedia("(prefers-color-scheme: dark)").matches ? <Theme>"dark" : <Theme>"light";
    } else {
      this._theme = scheme;
    }
    document.querySelector("html")?.setAttribute("data-theme", this.theme);
    window.localStorage?.setItem("pantryTheme", this.theme);
  }

  get theme(): Theme {
    return this._theme;
  }
}

const switcher = new ThemeSwitcher();
