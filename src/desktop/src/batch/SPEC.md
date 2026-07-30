# Batch Processing Specification

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.
>
> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate). See the [Requirements Index](../../SPEC.md#requirements-index) for the full REQ-xxx → file map.
>
> Owns BATCH-001..BATCH-014. Cross-cutting requirements that also touch this module are listed at the bottom of this file.

## Scope

This module owns the Batch Prompt Processing subsystem. It covers the batch processing dialog, directory/file selection, prompt selection, batch modes (File/Directory), concurrency control, and processing lifecycle. The code lives in `src/desktop/src/batch/`.

## Requirements

### Batch processing

* [BATCH-001] The system shall display a 'Batch ...' button on the top navigation/menu bar bar
* [BATCH-002] When the user clicks on the 'Batch ...' button, the [batch prompt processing dialog] opens
* [BATCH-003] The [batch prompt processing dialog] shall let the user select a directory from the available directories to process files in
* [BATCH-004] The [batch prompt processing dialog] shall let the user specify a wildcard pattern of file names to process
* [BATCH-005] The [batch prompt processing dialog] shall let the user select a prompt from a list of prompts. Prompts are markdown files with the 'prompt' tag
* [BATCH-006] The [batch prompt processing dialog] shall let the user choose between [Batch modes]. Batch modes are [File] and [Directory].
* [BATCH-007] The [batch prompt processing dialog] shall hide and ignore the contents of the wildcard pattern when the batch mode is [Directory], since it will not have control over which files are being processed.
* [BATCH-008] The [batch prompt processing dialog] shall let the user select a processing concurrency number. This shall be a drop-down box with available numbers from 1 to 8. The system shall process that number of prompts concurrently.
* [BATCH-009] When the user clicks the 'Cancel' button in the [batch prompt processing dialog], the system shall close the dialog with no action taken and no files modified
* [BATCH-010] When the user clicks the 'Process' button in the [batch prompt processing dialog], and the batch mode is [File], the system shall add the file context to the system context and process the prompt once per file.
* [BATCH-011] When the user clicks the 'Process' button in the [batch prompt processing dialog], and the batch mode is [Directory], the system shall add the directory context to the system context and process the prompt once per Directory.
* [BATCH-012] The [batch prompt processing dialog] shall log the start and end of LLM processing for each file to the background log window.
* [BATCH-013] While processing is underway, the [batch prompt processing dialog] shall disable the 'Process'
* [BATCH-014] While processing is underway, the [batch prompt processing dialog] shall stop processing new prompts when the user clicks the 'Cancel' button

## Cross-cutting references

- BATCH-001 — UI button on top panel lives in [`src/ui/SPEC.md`](../ui/SPEC.md)
- BATCH-012 — Background process logging lives in [`src/background/SPEC.md`](../background/SPEC.md)