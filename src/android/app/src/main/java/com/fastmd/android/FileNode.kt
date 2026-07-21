package com.fastmd.android

data class FileNode(
    val id: String,
    val name: String,
    val isDirectory: Boolean,
    var children: List<FileNode> = emptyList(),
    var downloadUrl: String? = null
)

object FileTreeProcessor {
    /**
     * Applies enumeration rules:
     * 1. Directories appear before files.
     * 2. Directory tree should not display folders that contain no markdown files.
     */
    fun processTree(root: FileNode): FileNode? {
        if (!root.isDirectory) {
            // It's a file. Only keep if it's a markdown file.
            return if (root.name.endsWith(".md", ignoreCase = true)) root else null
        }

        // It's a directory. Process children.
        val processedChildren = root.children.mapNotNull { processTree(it) }

        // Filter out empty directory
        if (processedChildren.isEmpty()) {
            return null 
        }
        
        // Sort: directories first, then alphabetically
        val sortedChildren = processedChildren.sortedWith(
            compareByDescending<FileNode> { it.isDirectory }.thenBy { it.name.lowercase(java.util.Locale.ROOT) }
        )
        
        return root.copy(children = sortedChildren)
    }
}
