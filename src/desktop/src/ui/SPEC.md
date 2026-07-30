# User interface specifications

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.

## Requirements for Table Layout and Renderer Subsystem

This document specifies the functional, algorithmic, and architectural requirements for a software Table Layout Engine and Renderer ("the System"). The System is responsible for taking structured tabular data and computing layout geometry, constraints, and visual representations.

### 1.1. Requirement Language

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC 2119] [RFC 8174] when, and only when, they appear in all capitals, as shown here.

---

## 2. Data Model & Input Specification

* [TBL-001] The System MUST accept input comprising tabular data organized into explicit rows and columns.
* [TBL-002] The System MUST support cell content consisting of arbitrary plain text.
* [TBL-003] The System MUST support formatted text as per the markdown specification.

## 3. Layout and Geometry Calculation

### 3.1. Sizing Constraints

* [TBL-010] The System MUST accept a maximum available target width ($W_{max}$) from the parent context.
* [TBL-011] The System MUST calculate the intrinsic minimum content width ($W_{min}$) and maximum preferred width ($W_{pref}$) for every column prior to final layout rendering.
* [TBL-012] If $W_{pref, total} \le W_{max}$, the System MUST allocate each column its preferred width $W_{pref}$.
* [TBL-013] If $W_{pref, total} > W_{max}$, the System MUST shrink column widths down to fit $W_{max}$, without reducing any column below its $W_{min}$, unless $W_{max} < W_{min, total}$.

### 3.2. Overflow and Wrapping

* [TBL-020] When column width allocation is less than cell content length, the System MUST wrap text content onto subsequent lines.
* [TBL-021] The System SHOULD break lines preferentially at whitespace characters.
* [TBL-022] If a single continuous word exceeds the allocated column width, the System MAY fallback to horizontal scrolling.

---

## 4. Alignment and Padding

* [TBL-030] The System MUST align horizontal content LEFT.
* [TBL-031] The System MUST align vertical content alignment within cells: TOP.
* [TBL-032] The System MAY have inner cell padding (top, bottom, left, right) on a per-cell, per-column, or global table level.
* [TBL-033] If present, Padding MUST be factored into all column width and row height calculations.

---

## 5. Rendering & Decoration

* [TBL-045] The System MUST render a medium-gray border around the outer perimeter of every markdown table — Width 1 px, color ≈ `Color32::from_gray(120)`.
* [TBL-044] The Renderer SHOULD NOT perform redundant re-layout passes if neither table data nor target viewport dimensions have changed.

---

## 6. Performance & Error Handling

* [TBL-50] Malformed input (e.g., inconsistent row lengths, negative padding values) MUST NOT result in undefined behavior or memory corruption. The System SHOULD return a descriptive error or normalize the input gracefully.
* [TBL-51] The system SHOULD use available resources (memory, threads, parellism etc) to speed up processing.
