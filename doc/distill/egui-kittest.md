# egui-kittest SDK Documentation

## Overview

**egui-kittest** is a GUI testing library for Rust, inspired by [Testing Library](https://testing-library.com/). It uses [AccessKit](https://github.com/AccessKit/accesskit/) to test user interfaces through the accessibility tree.

The documentation below is generated from the source in this repository, which is the **`kittest`** crate (version **0.4.0**). The **`egui_kittest`** crate is the official integration for [egui](https://github.com/emilk/egui). It depends on `kittest` and re-exports its API as `kittest` (for example: `use egui_kittest::{Harness, kittest::Queryable};`).

- **Crate name**: `kittest`
- **Version**: 0.4.0
- **License**: MIT OR Apache-2.0
- **Rust edition**: 2024
- **Minimum supported Rust version (MSRV)**: 1.92
- **Supported languages and frameworks**: Rust; any GUI framework that supports AccessKit (official integration: egui)
- **Key dependencies**: `accesskit` 0.24.0, `accesskit_consumer` 0.35.0
- **Repository**: <https://github.com/rerun-io/kittest>
- **Documentation generated on**: 2026-07-30

---

## Architecture & Core Concepts

The library has four layers. Each layer lives in its own module under `src/`.

| Module | File | Purpose |
|---|---|---|
| `State` | `src/state.rs` | Wraps the AccessKit tree. Holds the tree and applies tree updates. |
| `NodeT` | `src/node.rs` | A trait you implement for your framework's node type. Gives traversal methods. |
| `Queryable` | `src/query.rs` | A trait that provides query methods on any node or harness. |
| `By` | `src/filter.rs` | A filter struct. You combine filter conditions to find nodes. |

**Data flow**:

1. Your GUI framework produces an `accesskit::TreeUpdate` each frame.
2. `State::new` builds a `State` from the first `TreeUpdate`.
3. `State::update` applies every subsequent `TreeUpdate`.
4. `State::root` returns the root `AccessKitNode`.
5. Your test harness wraps the root node and implements `Queryable`.
6. You query nodes with the `query_*` / `get_*` methods, then interact with them.

**The node trait pattern**: kittest is framework-agnostic. In version 0.2.0, the concrete `kittest::Node` was replaced with the `NodeT` trait. You MUST implement `NodeT` for your own node type to unlock kittest functionality. This lets you add framework-native helpers (such as event dispatch and coordinate conversions) on your node type.

**Query semantics** (inspired by Testing Library):

- `query_*` returns `Option<Node>`. It returns `None` when nothing matches.
- `get_*` panics when nothing matches. Use it when a match is required.
- `query_*` and `get_*` (singular) panic when more than one node matches. Use the `_all` variants for multiple matches.
- All singular methods are `#[track_caller]`. Panic messages include the query and the node, so failures point to the test line.
- **Label nodes**: when a widget is labelled by another node, both nodes can share the same label. kittest excludes the labelling node from results, so only the labelled widget is returned. This matches the Testing Library `getByLabelText` behavior. Set `By::include_labels` to keep label nodes.

---

## API Reference

### Core Module / Class

#### `kittest::State`

##### Overview

A wrapper around [`accesskit_consumer::Tree`]. It holds the current accessibility tree of the tested application. You can also use `accesskit_consumer::Tree` directly.

##### Prerequisites & Requirements

- You MUST have an `accesskit::TreeUpdate` from your GUI framework before you construct a `State`.
- You MUST call `update` after every rendered frame to keep the tree in sync.
- The tree update MUST come from a framework that has AccessKit enabled (for egui: `egui::Context::enable_accesskit()`).

##### Syntax / Method Signature

```rust
pub struct State {
    // private field
}

impl State {
    pub fn new(update: TreeUpdate) -> Self;
    pub fn update(&mut self, update: accesskit::TreeUpdate);
    pub fn root(&self) -> AccessKitNode<'_>;
}

impl Debug for State; // renders a non-exhaustive debug struct
```

##### Examples

```rust
// Creation from the first frame output (egui integration example).
let ctx = egui::Context::default();
ctx.enable_accesskit();
let output = ctx.run_ui(Default::default(), &mut app);

let mut state = kittest::State::new(
    output.platform_output.accesskit_update.expect("AccessKit not enabled"),
);
```

##### Type references

- `accesskit::TreeUpdate`
- `accesskit_consumer::Tree`
- `AccessKitNode` (see below)

##### Return values

| Method | Returns |
|---|---|
| `new` | A new `State`. |
| `update` | Nothing (`()`). |
| `root` | The root `AccessKitNode<'_>` of the tree. |

##### Side effects

- `update` mutates the internal tree. It applies node additions, updates, removals, and focus moves. kittest ignores these changes (it uses a no-op change handler).
- `new` and `update` do not run the GUI itself. The framework MUST produce the `TreeUpdate` first.

##### References

- `src/state.rs`

---

#### `kittest::NodeT`

##### Overview

A trait for your test framework's node type. Implement it to make your nodes work with kittest. All querying and traversal in kittest operates through this trait.

##### Prerequisites & Requirements

- Your node type MUST implement `Clone` and `Debug`.
- Your node type MUST implement `accesskit_node` and `new_related`. The other methods have default implementations.
- `new_related` MUST construct a node that reuses the same framework handles as the parent node (for example, the same event queue). This keeps traversal working for related nodes.

##### Syntax / Method Signature

```rust
pub trait NodeT<'tree>: Clone + Debug {
    fn accesskit_node(&self) -> AccessKitNode<'tree>;
    fn new_related(&self, child_node: AccessKitNode<'tree>) -> Self;

    fn children_recursive(&self) -> Box<dyn Iterator<Item = Self> + 'tree> where Self: 'tree; // default impl
    fn children(&self) -> impl Iterator<Item = Self> + 'tree where Self: 'tree;               // default impl
    fn children_maybe_recursive(&self, recursive: bool) -> Box<dyn Iterator<Item = Self> + 'tree> where Self: 'tree; // default impl
    fn parent(&self) -> Option<Self>;                                                          // default impl
}
```

##### Examples

```rust
use kittest::{AccessKitNode, NodeT};

#[derive(Clone, Copy)]
pub struct EguiNode<'tree> {
    node: AccessKitNode<'tree>,
    queue: &'tree Mutex<Vec<egui::Event>>,
}

impl<'tree> NodeT<'tree> for EguiNode<'tree> {
    fn accesskit_node(&self) -> AccessKitNode<'tree> {
        self.node
    }

    fn new_related(&self, child_node: AccessKitNode<'tree>) -> Self {
        Self { queue: self.queue, node: child_node }
    }
}
```

##### Type references

- `AccessKitNode` (`accesskit_consumer::Node`)

##### Return values

| Method | Returns |
|---|---|
| `accesskit_node` | The wrapped `AccessKitNode<'tree>`. |
| `new_related` | A new instance of `Self` for a related node. |
| `children` | An iterator over direct child nodes. |
| `children_recursive` | An iterator over all descendant nodes, depth-first. |
| `children_maybe_recursive` | An iterator over children, recursive or not, based on the flag. |
| `parent` | The parent node, or `None` for the root. |

##### Side effects

- None. All methods are read-only.

##### References

- `src/node.rs:9`

---

#### `kittest::AccessKitNode`

##### Overview

A re-export of [`accesskit_consumer::Node`] under a more convenient name. It exposes the properties of one node in the accessibility tree.

##### Prerequisites & Requirements

- You MUST NOT construct one directly. You obtain nodes from `State::root`, or from traversal and query methods.

##### Syntax / Method Signature

```rust
pub use accesskit_consumer::Node as AccessKitNode;
```

Common methods (from `accesskit_consumer::Node`):

```rust
pub fn id(&self) -> NodeId;
pub fn role(&self) -> Role;
pub fn label(&self) -> Option<&str>;
pub fn value(&self) -> Option<&str>;
pub fn numeric_value(&self) -> Option<f64>;
pub fn toggled(&self) -> Option<Toggled>;
pub fn is_focused(&self) -> bool;
pub fn is_hidden(&self) -> bool;
pub fn is_disabled(&self) -> bool;
pub fn parent_id(&self) -> Option<NodeId>;
pub fn parent(&self) -> Option<Node<'_>>;
pub fn labelled_by(&self) -> impl Iterator<Item = Node<'_>>;
pub fn children(&self) -> impl Iterator<Item = Node<'_>>;
```

##### Examples

```rust
let checkbox = harness.get_by_label("Check me!");
assert_eq!(checkbox.toggled(), Some(Toggled::False));
let id = checkbox.accesskit_node().id();
let role = checkbox.accesskit_node().role();
```

##### Type references

- `accesskit::Role`, `accesskit::Toggled`, `accesskit::NodeId`

##### Return values

- `id` returns the node identifier.
- `role`, `label`, `value`, `numeric_value`, `toggled` return the node properties.
- `is_focused`, `is_hidden`, `is_disabled` return booleans.
- `parent` returns `Option<Node>`. `labelled_by` and `children` return iterators over nodes.

##### Side effects

- None.

##### References

- `src/lib.rs:9`

---

#### `kittest::Queryable`

##### Overview

A trait that provides convenience query methods on any node or harness. It is inspired by Testing Library. Implement `queryable_node` on your harness or node to unlock all query methods. There is a blanket implementation for all types that implement `NodeT`.

##### Prerequisites & Requirements

- Your type MUST implement `queryable_node`.
- For a harness, the returned node MUST be the root node of the tree.
- All `&'tree str` arguments MUST live at least as long as the tree.

##### Syntax / Method Signature

```rust
pub trait Queryable<'tree, 'node, Node: NodeT<'tree> + 'tree> {
    fn queryable_node(&'node self) -> Node;
}
```

**Query methods** (each in four variants: `query_all_*`, `get_all_*`, `query_*`, `get_*`):

| Base name | Filter | Extra docs |
|---|---|---|
| `query_all(by)` / `get_all(by)` / `query(by)` / `get(by)` | `By` filter | |
| `query_all_by_label(label)` / `get_all_by_label(label)` / `query_by_label(label)` / `get_by_label(label)` | exact label match | excludes label nodes |
| `query_all_by_label_contains(label)` / `get_all_by_label_contains(label)` / `query_by_label_contains(label)` / `get_by_label_contains(label)` | substring label match | excludes label nodes |
| `query_all_by_role_and_label(role, label)` / `get_all_by_role_and_label(role, label)` / `query_by_role_and_label(role, label)` / `get_by_role_and_label(role, label)` | role and exact label | excludes label nodes |
| `query_all_by_role(role)` / `get_all_by_role(role)` / `query_by_role(role)` / `get_by_role(role)` | role match | |
| `query_all_by_value(value)` / `get_all_by_value(value)` / `query_by_value(value)` / `get_by_value(value)` | exact value match | |
| `query_all_by(f)` / `get_all_by(f)` / `query_by(f)` / `get_by(f)` | custom predicate | |

All `_all` and `query_all` variants return:

```rust
impl DoubleEndedIterator<Item = Node> + FusedIterator<Item = Node> + 'tree
```

The `_all` variants MUST return at least one node; they panic otherwise.

##### Examples

```rust
use kittest::{Queryable, by};

// Optional query: returns None when nothing matches.
let button = harness.query_by_label("Button 1");

// Required query: panics when nothing matches.
let button = harness.get_by_label("Button 2");

// Multiple matches: use the _all variant.
assert_eq!(harness.query_all_by_label("Duplicate").count(), 2);

// Disambiguate with a role.
let submit = harness.get_by_role_and_label(Role::Button, "Submit");

// Compound query via the By struct.
let check_me = harness.get(by().role(Role::CheckBox).label_contains("Check"));

// Query within a subtree: call the method on a node, not the harness.
let group = harness.get_by_role_and_label(Role::GenericContainer, "My Group");
group.get_by_label("Duplicate");
```

##### Type references

- `By`, `NodeT`, `AccessKitNode`

##### Return values

- `query_*` returns `Option<Node>`.
- `get_*` returns `Node`.
- `query_all_*` and `get_all_*` return a double-ended, fused iterator of `Node`.

##### Side effects

- None.

##### References

- `src/query.rs:147`

---

#### `kittest::By`

##### Overview

A filter for nodes. All active conditions MUST match for a node to pass the filter. The conditions are combined with a logical AND.

##### Prerequisites & Requirements

- You MUST construct a filter with `By::new()` or the `by()` helper.
- Filter methods consume and return `self`, so you MUST chain them.
- You MUST pass the filter to a `Queryable` method (`query_all`, `get`, and so on) for it to take effect.

##### Syntax / Method Signature

```rust
#[derive(Clone)]
pub struct By<'a> { /* private fields */ }

impl<'a> By<'a> {
    pub fn new() -> Self;
    pub fn label(mut self, label: &'a str) -> Self;            // exact match
    pub fn label_contains(mut self, label: &'a str) -> Self;   // substring match
    pub fn include_labels(mut self) -> Self;                   // include label nodes
    pub fn predicate(mut self, predicate: impl Fn(&AccessKitNode<'_>) -> bool + 'a) -> Self;
    pub fn role(mut self, role: Role) -> Self;
    pub fn value(mut self, value: &'a str) -> Self;            // exact match
    pub fn recursive(mut self, recursive: bool) -> Self;       // default: true
}

impl Default for By<'_>; // returns By::new()
```

##### Examples

```rust
use kittest::{Queryable, by};
use accesskit::Role;

let check_me = harness.get(
    by()
        .role(Role::CheckBox)
        .label_contains("Check")
        .recursive(true),
);

// Predicates receive the underlying AccessKit node.
harness.get_all(by().predicate(|node| node.is_disabled()));
```

##### Type references

- `accesskit::Role`, `AccessKitNode`

##### Return values

| Method | Returns |
|---|---|
| `new` / `Default` | An empty `By` filter that matches everything. |
| `label`, `label_contains`, `include_labels`, `predicate`, `role`, `value`, `recursive` | The modified `By` instance. |

##### Side effects

- None.

##### Notes on behavior

- `recursive` defaults to `true`. The search covers the whole subtree. Set `recursive(false)` to search only direct children.
- **`Role::Label` special case**: in AccessKit, a widget with `Role::Label` stores its label in `Node::value`. kittest checks for this and reads `value` when the role is `Role::Label`.
- `include_labels` defaults to `false`. When `false`, nodes that only act as labels for other nodes are excluded from results.
- `By` implements `Clone` and `Debug`. The `Debug` output is used in panic messages for failed queries.

##### References

- `src/filter.rs:15`

---

### Utility Functions

#### `kittest::by`

##### Overview

A convenience function that creates an empty filter. It is equivalent to `By::new()`.

##### Prerequisites & Requirements

- None.

##### Syntax / Method Signature

```rust
pub fn by<'a>() -> By<'a>;
```

##### Examples

```rust
let filter = by().role(Role::Button);
let buttons = harness.query_all(filter);
```

##### Type references

- `By`

##### Return values

- A new empty `By<'a>` filter.

##### Side effects

- None.

##### References

- `src/filter.rs:8`

---

#### `kittest::debug_fmt_node`

##### Overview

A helper function to format an AccessKit node and its children as a `Debug` string. Use it in your node's `Debug` implementation to get readable recursive output.

##### Prerequisites & Requirements

- The node type MUST implement `NodeT`.

##### Syntax / Method Signature

```rust
pub fn debug_fmt_node<'tree, Node: NodeT<'tree> + 'tree>(
    node: &Node,
    f: &mut Formatter<'_>,
) -> std::fmt::Result;
```

##### Examples

```rust
impl Debug for EguiNode<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        debug_fmt_node(self, f)
    }
}
```

##### Type references

- `NodeT`, `std::fmt::Formatter`

##### Return values

- `std::fmt::Result`.

##### Side effects

- Writes the formatted node tree into the formatter. The output includes `id`, `role`, `label`, `value`, `numeric_value`, `focused`, `hidden`, `disabled`, `toggled`, and `children` (recursively). Fields with no value are omitted.

##### References

- `src/node.rs:69`

---

## Configuration Reference

The `kittest` crate has no runtime configuration. Its only feature is the default feature set; there are no optional features.

### Cargo dependency configuration

```toml
[dependencies]
kittest = "0.4.0"

# For egui integration, depend on egui_kittest instead:
# egui_kittest = { version = "0.x", features = ["wgpu"] }  # check the egui crate for the current API
```

- The crate MUST be used with Rust 1.92 or newer.
- The crate MUST be used with the 2024 edition or newer.
- A `[patch.crates-io]` entry pins `egui` to the `main` branch during development. A released consumer MUST NOT rely on this patch.

### MSRV and edition

| Setting | Value |
|---|---|
| `rust-version` | 1.92 |
| `edition` | 2024 |

---

## Error Handling

kittest does not return `Result` values. It uses panics for all failure modes. This is intentional: a failed test SHOULD stop immediately with a clear message.

### Panic conditions

| Situation | Behavior |
|---|---|
| `get_*` / `get_all_*` finds no matching node | Panics with the message `No nodes found matching the query:`, followed by the query and the node. |
| `query_*` / `get_*` (singular) finds two or more matching nodes | Panics with `Found two or more nodes matching the query:`, lists the first and second nodes, and suggests using `query_all` instead. |
| `debug_fmt_node` fails to format | Returns `std::fmt::Error` (the only error-like return in the crate). |

All query methods use `#[track_caller]`, so the panic location points to the failing test assertion in your code.

### Panic messages

- All panic messages include a `Debug` rendering of the query and the node. This makes the failing test easier to diagnose.
- `By` implements `Debug`. Predicates are shown as `<function>`.

### Recommended error-handling pattern

```rust
// Use query_* when a node may legitimately be absent.
if let Some(button) = harness.query_by_label("Submit") {
    button.click();
    harness.run();
}

// Use get_* when the node must exist; the panic is the test failure.
let submit = harness.get_by_role_and_label(Role::Button, "Submit");
```

---

## Integration Guide

To integrate kittest with your own AccessKit-capable framework, follow the pattern in `integration_example/src/bin/basic_integration.rs`.

### Steps

1. Create a harness struct that holds your framework context, your app, and a `kittest::State`.
2. Enable AccessKit in your framework.
3. Run one frame, take `platform_output.accesskit_update`, and pass it to `kittest::State::new`.
4. After each subsequent frame, pass the new `TreeUpdate` to `state.update`.
5. Define a node type and implement `NodeT` for it.
6. Implement `Queryable` for the harness. Return the root node from `queryable_node`.
7. Add input helpers (such as `click`) to your node type. Queue framework events; they are applied on the next `run` / frame.

```rust
// Minimal harness skeleton.
pub struct Harness<'a> {
    ctx: egui::Context,
    app: Box<dyn FnMut(&mut egui::Ui) + 'a>,
    pub state: kittest::State,
    queued_events: Mutex<Vec<egui::Event>>,
}

impl<'tree, 'node> Queryable<'tree, 'node, EguiNode<'tree>> for Harness<'_> {
    fn queryable_node(&'node self) -> EguiNode<'tree> {
        EguiNode { queue: &self.queued_events, node: self.state.root() }
    }
}
```

### Example usage with egui_kittest

```rust
use egui::accesskit::Toggled;
use egui_kittest::{Harness, kittest::Queryable};

fn main() {
    let mut checked = false;
    let app = |ui: &mut egui::Ui| {
        ui.checkbox(&mut checked, "Check me!");
    };

    let mut harness = Harness::new_ui(app);

    let checkbox = harness.get_by_label("Check me!");
    assert_eq!(checkbox.toggled(), Some(Toggled::False));
    checkbox.click();

    harness.run();

    let checkbox = harness.get_by_label("Check me!");
    assert_eq!(checkbox.toggled(), Some(Toggled::True));
}
```

### Querying examples

- `harness.query_by_label(...)` returns `Option<Node>`.
- `harness.get_by_label(...)` panics when the node is not found.
- `harness.query_all_by_label(...)` returns an iterator over all matches.
- `harness.get_by_role_and_label(Role::Button, "Submit")` disambiguates nodes by role.
- `harness.get(by().role(...).label_contains(...))` builds a compound filter.
- Call query methods on a node to scope the search to its subtree.

Run the integration examples with:

```bash
cargo run -p integration_example --bin basic_integration
cargo run -p integration_example --bin querying
```

---

## Additional Resources

- **kittest README**: `README.md`
- **Changelog**: `CHANGELOG.md`
- **Integration example**: `integration_example/src/bin/basic_integration.rs`
- **Querying example**: `integration_example/src/bin/querying.rs`
- **egui_kittest integration**: <https://github.com/emilk/egui/tree/master/crates/egui_kittest>
- **AccessKit**: <https://github.com/AccessKit/accesskit/>
- **Testing Library** (design inspiration): <https://testing-library.com/>
