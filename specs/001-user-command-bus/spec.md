# Feature Specification: User Command Bus

**Feature Branch**: `[001-user-command-bus]`

**Created**: 2026-09-01

**Status**: Draft

**Input**: User description: "Introduce a `Bus<UserCommand>` broadcast channel and a unified `UserCommand` enum so every user input (toolbar, command line, tabs, TOC, file tree, modals, keyboard shortcuts) flows through one typed intake, with execution centralised in an orchestrator-side executor. This decouples UI panels from `AppOrchestrator` state mutation, retires the `submit_prompt: Option<String>` deferred-action slot, and preserves the existing Tier-4 click-capture testability of the `apply_*` helpers."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Centralized User Intent Routing (Priority: P1)

As a developer maintaining the application, I want all user inputs to flow through a single typed event bus, so that I can easily log, trace, and replay user commands without digging into UI rendering code.

**Why this priority**: Centralizing user input is the core architectural goal of this feature.
**Independent Test**: Can be fully tested by verifying that any UI interaction produces an event on the `Bus<UserCommand>`.

**Acceptance Scenarios**:

1. **Given** the application is running, **When** a user interacts with any UI element (e.g., clicking a toolbar button), **Then** a corresponding `UserCommand` event is published to the bus.

---

### User Story 2 - Isolated UI State Rendering (Priority: P1)

As an application architect, I want UI components to only publish intents rather than mutate application state, so that rendering logic is decoupled from business logic and state consistency is maintained.

**Why this priority**: Decoupling the UI from state mutation prevents complex snapshot/flush hazards and reduces tight coupling.
**Independent Test**: Can be tested by verifying that UI panels no longer require mutable borrows of the application orchestrator.

**Acceptance Scenarios**:

1. **Given** a UI panel rendering function, **When** it receives a user action, **Then** it strictly publishes a command and does not directly alter core application data structures.

---

### User Story 3 - Independent UI Click Testing (Priority: P2)

As a QA engineer, I want the UI interaction tests to remain independent of the full application state, so that I can quickly verify user intent capture without setting up complex mock state.

**Why this priority**: Preserving existing testability (Tier-4 tests) ensures that architectural changes do not degrade confidence in the UI.
**Independent Test**: Can be tested by running Tier-4 click-capture tests and asserting on the returned `UserCommand`.

**Acceptance Scenarios**:

1. **Given** a Tier-4 click test, **When** a simulated click occurs, **Then** the test successfully asserts the correct `UserCommand` was produced without needing orchestrator state.

### Edge Cases

- What happens if the `Bus<UserCommand>` reader lags or is overloaded? (Lagged reader should drop or log events gracefully according to existing bus strategies).
- How are high-frequency UI events handled without flooding the command bus?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST route all user UI interactions through a unified `Bus<UserCommand>` broadcast channel.
- **FR-002**: System MUST define a comprehensive `UserCommand` enum containing variants for all supported user interactions (e.g., RunAgent, CloseTab, etc.).
- **FR-003**: System MUST NOT allow UI rendering components to mutate the central application or orchestrator state directly.
- **FR-004**: System MUST centralize the processing of user actions in an orchestrator-side command executor that drains the `Bus<UserCommand>`.
- **FR-005**: System MUST eliminate the `submit_prompt` deferred-action slot in favor of the event bus pattern.
- **FR-006**: System MUST preserve existing Tier-4 test assertions by validating published `UserCommand` objects instead of side effects.

### Key Entities

- **UserCommand**: A strongly-typed enum representing a discrete user action, containing all necessary payload data to execute the action.
- **CommandExecutor**: A centralized component within the orchestrator responsible for interpreting `UserCommand` events and applying the resulting mutations to application state.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of user interactions are dispatched as `UserCommand` events on the bus instead of inline mutations.
- **SC-002**: Zero UI-side `&mut AppOrchestrator` borrows exist for handling user actions.
- **SC-003**: `submit_prompt` is completely removed from the orchestrator state structure.
- **SC-004**: All Tier-4 click-capture tests pass successfully by asserting on published commands.

## Assumptions

- Execution of commands is synchronous within the orchestrator's frame loop processing.
- No existing user features or capabilities are removed; only the internal architecture changes.
- `UserCommand` variants contain fully cloned/owned data and do not borrow UI state.
