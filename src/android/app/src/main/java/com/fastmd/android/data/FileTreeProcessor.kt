package com.fastmd.android.data

/**
 * Applies enumeration rules:
 * 1. Files that do not end in `.md` (case-insensitive) are dropped.
 * 2. Directories that contain no markdown files (after their own children
 *    are filtered) are dropped.
 * 3. Within a directory, children are sorted: directories first, then
 *    files, each group sorted alphabetically (case-insensitive).
 *
 * Faithful Kotlin port of `FileTreeProcessor` from the original
 * `MainActivity.kt`. The four regression tests in
 * `app/src/test/java/com/fastmd/android/data/FileTreeProcessorTest.kt`
 * mirror `tests/file_tree_processor.rs` in the egui port and must stay
 * in lockstep.
 */
object FileTreeProcessor {
    fun processTree(root: FileNode): FileNode? {
        if (!root.isDirectory) {
            return if (isMarkdown(root.name)) root else null
        }

        val processedChildren = root.children.mapNotNull { processTree(it) }

        if (processedChildren.isEmpty()) {
            return null
        }

        val sortedChildren = processedChildren.sortedWith(
            compareByDescending<FileNode> { it.isDirectory }
                .thenBy { it.name.lowercase(java.util.Locale.ROOT) },
        )

        return root.copy(children = sortedChildren)
    }

    private fun isMarkdown(name: String): Boolean {
        val ext = name.substringAfterLast('.', missingDelimiterValue = "")
        return ext.equals("md", ignoreCase = true) && ext.length != name.length
    }
}
