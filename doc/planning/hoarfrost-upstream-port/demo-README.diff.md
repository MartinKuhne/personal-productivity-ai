# Demo `README.md` — local vs. upstream

> **Status:** Port record — captured 2026-11-01 from the working tree, **not yet
> applied upstream**. Companion to
> [`hoarfrost-external-crate-plan.md`](../hoarfrost-external-crate-plan.md) §2.2.

The plan notes only that the demo `README.md` "differs" upstream. This records
*how*, so nothing is silently lost when the local copy is deleted in Phase 2.

## How this diff was produced

```
git diff --no-index -- src\hoarfrost-demo\README.md D:\hoarfrost\src\hoarfrost-demo\README.md
```

`a/` is **this repo** (`D:\ppai2`), `b/` is the **upstream** checkout
(`D:\hoarfrost`). The upstream side is the committed `dd7af16` blob
(`2326ca6bb0332ae01a1713ec007911b5b946cdcd`, file clean in `git status`); this
repo's file is `9d4a36a326c0acd60b0fdf0decbca8db02904f3e`.

## The diff (verbatim)

```diff
diff --git "a/src\\hoarfrost-demo\\README.md" "b/D:\\hoarfrost\\src\\hoarfrost-demo\\README.md"
index 9d4a36a..2326ca6 100644
--- "a/src\\hoarfrost-demo\\README.md"
+++ "b/D:\\hoarfrost\\src\\hoarfrost-demo\\README.md"
@@ -280,9 +280,3 @@ drifting quietly.
   so the section list never had a placeholder to be replaced later.
 
 ---
-
-## Bugs found
-
-See **[BUGS.md](BUGS.md)** for the full triage: one framework robustness
-gap with a runnable canary, one demo mis-composition the layout gate
-caught, and five framework behaviours that looked like gaps and were not.
```

## What it means

One hunk, six deleted lines, no additions: this repo's `README.md` ends with a
`## Bugs found` section that points at `BUGS.md`. Upstream dropped it **because
upstream has no `BUGS.md`** — the section was removed together with the file, not
because the content was rejected.

The rest of the two files is byte-identical.

## The dangling references upstream already has

Upstream's `README.md` still points at two files that do not exist upstream:

| Upstream line | Reference |
|---|---|
| `src/hoarfrost-demo/README.md:167` | `` `BUGS.md` `` |
| `src/hoarfrost-demo/README.md:206` | "for the framework finding in `BUGS.md`" |
| `src/hoarfrost-demo/README.md:274` | heading "Deviations from `PLAN.md` §3" |

So porting `BUGS.md` and `PLAN.md` upstream (Phase 0.1) does more than preserve
them — it repairs three references that are dead upstream today. When that lands,
restore this trailing `## Bugs found` section too, so the triage log is
discoverable from the README rather than only from grep.
