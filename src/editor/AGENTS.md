# Editor Module

- Select dropdowns in forms: always prepend a disabled hidden `<option value=''>` placeholder so the `change` event fires correctly when picking a real option. Without this, if `data[key]` is empty, the browser auto-selects the first option without triggering the event, and form data stays stale.
- Each page module exports `renderPage()` returning a DOM element; inline edit panels append to the root container with `scrollIntoView`.
- i18n: use `t(key, params)` from `../../i18n.js`, fire `langchange` custom event for dynamic re-render.
