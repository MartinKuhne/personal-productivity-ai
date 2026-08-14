package com.fastmd.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Regression tests for [FileTreeProcessor]. The four scenarios here were
 * ported from the original `AppTest.kt` and mirror the four tests in the
 * Rust egui port at `src/android.egui/tests/file_tree_processor.rs`.
 * Any change to the filter or sort order must update both files in
 * lockstep.
 */
class FileTreeProcessorTest {

    @Test
    fun `filters out non-markdown files`() {
        val root = dir(
            "root",
            listOf(
                file("image.png"),
                file("document.txt"),
                file("notes.md"),
            ),
        )

        val processed = FileTreeProcessor.processTree(root)

        assertNotNull(processed)
        assertEquals(1, processed!!.children.size)
        assertEquals("notes.md", processed.children[0].name)
    }

    @Test
    fun `filters out empty directories`() {
        val root = dir(
            "root",
            listOf(
                dir("EmptyDir", listOf(file("image.png"))),
                dir("ValidDir", listOf(file("valid.md"))),
            ),
        )

        val processed = FileTreeProcessor.processTree(root)

        assertNotNull(processed)
        assertEquals(1, processed!!.children.size)
        assertEquals("ValidDir", processed.children[0].name)
    }

    @Test
    fun `returns null if root is empty directory`() {
        val root = dir("root", listOf(file("image.png")))

        val processed = FileTreeProcessor.processTree(root)

        assertNull(processed)
    }

    @Test
    fun `sorts directories before files`() {
        val root = dir(
            "root",
            listOf(
                file("z_file.md"),
                dir("a_dir", listOf(file("doc.md"))),
                file("a_file.md"),
                dir("z_dir", listOf(file("doc.md"))),
            ),
        )

        val processed = FileTreeProcessor.processTree(root)

        assertNotNull(processed)
        assertEquals(4, processed!!.children.size)
        // Directories first, sorted alphabetically.
        assertEquals("a_dir", processed.children[0].name)
        assertEquals("z_dir", processed.children[1].name)
        // Then files, sorted alphabetically.
        assertEquals("a_file.md", processed.children[2].name)
        assertEquals("z_file.md", processed.children[3].name)
    }

    @Test
    fun `is markdown by extension check handles edge cases`() {
        // The processor's isMarkdown helper is private, but we can
        // exercise it through the public contract: a file with no
        // extension and a file with a different extension are both
        // dropped.
        val root = dir(
            "root",
            listOf(
                file("README"),          // no extension
                file("notes.markdown"),  // wrong extension
                file("notes.md"),        // OK
                file("notes.MD"),        // OK, case-insensitive
            ),
        )
        val processed = FileTreeProcessor.processTree(root)!!
        assertEquals(listOf("notes.md", "notes.MD"), processed.children.map { it.name })
    }

    private fun file(name: String): FileNode = FileNode(
        id = "file-$name",
        name = name,
        isDirectory = false,
    )

    private fun dir(name: String, children: List<FileNode>): FileNode = FileNode(
        id = "dir-$name",
        name = name,
        isDirectory = true,
        children = children,
    )
}
