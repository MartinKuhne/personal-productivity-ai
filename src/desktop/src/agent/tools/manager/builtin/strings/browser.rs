//! User-visible description strings for the headless browser tool family.
//!
//! Drives the eight tools (`browser_navigate`, `browser_get_page_state`,
//! `browser_click`, `browser_fill_input`, `browser_select_dropdown`,
//! `browser_press_key`, `browser_evaluate_js`, `browser_screenshot`)
//! registered under `InternalToolGroup::Browser`. See
//! `doc/planning/browser_tools.md` for the design record.

// --- browser_navigate ---

pub const BROWSER_NAVIGATE_DESCRIPTION: &str = "Navigate the headless browser to a URL. The system preserves page state across calls for multi-step flows. The response returns the final URL and page title.";

// --- browser_get_page_state ---

pub const BROWSER_GET_PAGE_STATE_DESCRIPTION: &str = "Get interactable elements (a, button, input, select, textarea), current URL, and page title. Each element includes a stable agent_id for action tools.";

// --- browser_click ---

pub const BROWSER_CLICK_DESCRIPTION: &str = "Click an element on the page using a CSS selector. Subsequent page state calls reflect the updated DOM.";

// --- browser_fill_input ---

pub const BROWSER_FILL_INPUT_DESCRIPTION: &str = "Fill an input or textarea element with text. This action replaces any existing value. Press Enter using browser_press_key to submit forms.";

// --- browser_select_dropdown ---

pub const BROWSER_SELECT_DROPDOWN_DESCRIPTION: &str =
    "Select an option in a dropdown element using its value attribute.";

// --- browser_press_key ---

pub const BROWSER_PRESS_KEY_DESCRIPTION: &str =
    "Press a keyboard key on the page (such as Enter, Tab, Escape, or ArrowDown).";

// --- browser_evaluate_js ---

pub const BROWSER_EVALUATE_JS_DESCRIPTION: &str = "Evaluate a JavaScript expression in the page context. The tool serializes the return value to JSON.";

// --- browser_screenshot ---

pub const BROWSER_SCREENSHOT_DESCRIPTION: &str = "Save a PNG screenshot of the page to the configured directory. The tool restricts filenames to valid alphanumeric characters (up to 128 characters).";

// --- Field descriptions ---

pub const FIELD_BROWSER_NAVIGATE_INPUT_URL: &str =
    "Specify the absolute URL to navigate to (such as `https://example.com/login`).";

pub const FIELD_BROWSER_NAVIGATE_RESPONSE_URL: &str =
    "The final URL after navigation. This URL changes if the server redirects the request.";

pub const FIELD_BROWSER_NAVIGATE_RESPONSE_TITLE: &str =
    "The page `<title>` after navigation. Returns an empty string if the page has no title.";

pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_URL: &str = "The current page URL.";

pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TITLE: &str =
    "The current page `<title>`. Returns an empty string if the page has no title.";

pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_ELEMENTS: &str = "JSON array of interactable elements. Each entry contains `agent_id`, `tag`, `text`, `placeholder`, `name`, and `type`. Use `agent_id` to target specific elements.";

pub const FIELD_BROWSER_GET_PAGE_STATE_RESPONSE_TOTAL: &str =
    "Total number of interactable elements on the page.";

pub const FIELD_BROWSER_CLICK_INPUT_SELECTOR: &str = "Specify the CSS selector for the element to click. Use `agent_id` to build a `:nth-of-type(...)` selector if needed.";

pub const FIELD_BROWSER_FILL_INPUT_INPUT_SELECTOR: &str =
    "Specify the CSS selector for the input or textarea to fill.";

pub const FIELD_BROWSER_FILL_INPUT_INPUT_TEXT: &str =
    "Provide the text to insert. Replaces any existing text.";

pub const FIELD_BROWSER_SELECT_DROPDOWN_INPUT_SELECTOR: &str =
    "Specify the CSS selector for the `<select>` element.";

pub const FIELD_BROWSER_SELECT_DROPDOWN_INPUT_VALUE: &str =
    "Specify the `value` attribute of the `<option>` to select.";

pub const FIELD_BROWSER_PRESS_KEY_INPUT_KEY: &str =
    "Specify the key to press (such as `Enter`, `Tab`, `Escape`, or `ArrowDown`).";

pub const FIELD_BROWSER_EVALUATE_JS_INPUT_SCRIPT: &str = "Provide a JavaScript expression to evaluate in the page context. The tool serializes the return value to JSON.";

pub const FIELD_BROWSER_SCREENSHOT_INPUT_FILENAME: &str = "Specify the filename for the PNG. Must match `[A-Za-z0-9._-]{1,128}` without path separators or leading dots.";

pub const FIELD_BROWSER_SCREENSHOT_INPUT_FULL_PAGE: &str = "Set to `true` to capture the entire scrollable page, or `false` to capture only the current viewport. Default: `false`.";

pub const FIELD_BROWSER_SCREENSHOT_RESPONSE_PATH: &str =
    "Absolute path of the saved PNG screenshot file.";

pub const FIELD_BROWSER_SCREENSHOT_RESPONSE_BYTES: &str =
    "Size of the saved PNG screenshot file in bytes.";
