//! User-visible description strings for the headless browser tool family.
//!
//! Drives the eight tools (`browser_navigate`, `browser_get_page_state`,
//! `browser_click`, `browser_fill_input`, `browser_select_dropdown`,
//! `browser_press_key`, `browser_evaluate_js`, `browser_screenshot`)
//! registered under `InternalToolGroup::Browser`. See
//! `doc/planning/browser_tools.md` for the design record.

// --- browser_navigate ---

pub const BROWSER_NAVIGATE_DESCRIPTION: &str = "Drive the persistent headless Firefox page to a URL. The page state \
     (cookies, JS state, scroll position) is preserved across calls so the \
     LLM can drive multi-step flows. After navigation, the response includes \
     the final `url` and the page `title` (BRWS-001).";

// --- browser_get_page_state ---

pub const BROWSER_GET_PAGE_STATE_DESCRIPTION: &str = "Return the interactable elements on the current page (a, button, input, \
     select, textarea) plus the current `url` and `title`. Each element gets a \
     stable `agent_id` you can pass back to `browser_click` / \
     `browser_fill_input`. This is the only ReadOnly browser tool, safe to \
     call alongside other read-only tools (BRWS-002).";

// --- browser_click ---

pub const BROWSER_CLICK_DESCRIPTION: &str = "Click a single element on the page, identified by a CSS selector. The \
     page state changes; subsequent `browser_get_page_state` calls reflect the \
     post-click DOM (BRWS-003).";

// --- browser_fill_input ---

pub const BROWSER_FILL_INPUT_DESCRIPTION: &str = "Fill a single <input> or <textarea> with the given text. Replaces any \
     existing value. Use `browser_press_key` afterwards to submit if the form \
     listens for Enter (BRWS-004).";

// --- browser_select_dropdown ---

pub const BROWSER_SELECT_DROPDOWN_DESCRIPTION: &str = "Select an option in a <select> by its `value` attribute. The page state \
     changes (BRWS-005).";

// --- browser_press_key ---

pub const BROWSER_PRESS_KEY_DESCRIPTION: &str = "Press a single keyboard key on the page (e.g. `Enter`, `Tab`, `Escape`, \
     `ArrowDown`). Useful for form submission or modal dismissal (BRWS-006).";

// --- browser_evaluate_js ---

pub const BROWSER_EVALUATE_JS_DESCRIPTION: &str = "Evaluate an arbitrary JavaScript expression in the page context. Return \
     value is serialised to JSON (use `null` to indicate absence). This is a \
     true escape hatch — any side effect the page can do, this tool can do. \
     The page state may change (BRWS-007).";

// --- browser_screenshot ---

pub const BROWSER_SCREENSHOT_DESCRIPTION: &str = "Save a PNG screenshot of the current page to the screenshot directory \
     configured under `browser.screenshot_dir`. The `filename` argument is \
     sanitised: only `[A-Za-z0-9._-]`, no `..`, no path separators, ≤ 128 \
     chars, must not start with `.`. The screenshot path is restricted to the \
     configured directory; the LLM cannot write outside it (BRWS-008).";

// --- Field descriptions ---

pub const FIELD_BROWSER_NAVIGATE_INPUT_URL: &str =
    "Absolute URL to navigate to (e.g. `https://example.com/login`).";

pub const FIELD_BROWSER_NAVIGATE_RESPONSE_URL: &str = "Final URL after navigation (may differ from the input if the server \
     redirected).";

pub const FIELD_BROWSER_NAVIGATE_RESPONSE_TITLE: &str =
    "The page `<title>` after navigation. Empty string if the page has no title.";

pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_URL: &str = "Current page URL.";

pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TITLE: &str =
    "Current page `<title>`. Empty string if the page has no title.";

pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_ELEMENTS: &str = "JSON array of interactable elements (a, button, input, select, \
     textarea). Each entry has `agent_id`, `tag`, `text`, `placeholder`, \
     `name`, `type`. Use `agent_id` to disambiguate when many similar \
     elements are present (the `selector` for `browser_click` may need a \
     `:nth-of-type(...)` if there is no unique CSS path).";

pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TOTAL: &str = "Total number of interactable elements on the page (i.e. the length of \
     the `elements` array).";

pub const FIELD_BROWSER_CLICK_INPUT_SELECTOR: &str = "CSS selector for the element to click. Use the `agent_id` reported by \
     `browser_get_page_state` to build a `:nth-of-type(...)` selector if \
     no unique CSS path exists.";

pub const FIELD_BROWSER_FILL_INPUT_INPUT_SELECTOR: &str =
    "CSS selector for the input or textarea to fill.";

pub const FIELD_BROWSER_FILL_INPUT_INPUT_TEXT: &str =
    "Text to insert. Replaces any existing value.";

pub const FIELD_BROWSER_SELECT_DROPDOWN_INPUT_SELECTOR: &str =
    "CSS selector for the <select> element.";

pub const FIELD_BROWSER_SELECT_DROPDOWN_INPUT_VALUE: &str =
    "`value` attribute of the <option> to select.";

pub const FIELD_BROWSER_PRESS_KEY_INPUT_KEY: &str = "Key to press (e.g. `Enter`, `Tab`, `Escape`, `ArrowDown`). See Playwright \
     `page.keyboard.press` for the full set.";

pub const FIELD_BROWSER_EVALUATE_JS_INPUT_SCRIPT: &str = "JavaScript expression to evaluate in the page context. May be an \
     expression (`document.title`) or an arrow function \
     (`() => document.title`). Return value is serialised to JSON.";

pub const FIELD_BROWSER_SCREENSHOT_INPUT_FILENAME: &str = "Filename for the PNG (no path). Must match `[A-Za-z0-9._-]{1,128}` and \
     not start with `.` or contain `..`.";

pub const FIELD_BROWSER_SCREENSHOT_INPUT_FULL_PAGE: &str = "If `true`, capture the entire scrollable page, not just the viewport. \
     Defaults to `false`.";

pub const FIELD_BROWSER_SCREENSHOT_RESPONSE_PATH: &str =
    "Absolute path of the saved PNG. Always inside `browser.screenshot_dir`.";

pub const FIELD_BROWSER_SCREENSHOT_RESPONSE_BYTES: &str = "Size of the saved PNG in bytes.";
