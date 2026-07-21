package com.fastmd.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AppTest {

    @Test
    fun testFileTreeProcessor_filtersOutNonMarkdownFiles() {
        val root = FileNode(
            id = "root",
            name = "root",
            isDirectory = true,
            children = listOf(
                FileNode("1", "image.png", false),
                FileNode("2", "document.txt", false),
                FileNode("3", "notes.md", false)
            )
        )

        val processed = FileTreeProcessor.processTree(root)
        
        assertEquals(1, processed?.children?.size)
        assertEquals("notes.md", processed?.children?.first()?.name)
    }

    @Test
    fun testFileTreeProcessor_filtersOutEmptyDirectories() {
        val root = FileNode(
            id = "root",
            name = "root",
            isDirectory = true,
            children = listOf(
                FileNode(
                    id = "emptyDir",
                    name = "EmptyDir",
                    isDirectory = true,
                    children = listOf(
                        FileNode("1", "image.png", false) // Will be filtered out, making directory empty
                    )
                ),
                FileNode(
                    id = "validDir",
                    name = "ValidDir",
                    isDirectory = true,
                    children = listOf(
                        FileNode("2", "valid.md", false)
                    )
                )
            )
        )

        val processed = FileTreeProcessor.processTree(root)
        
        assertEquals(1, processed?.children?.size)
        assertEquals("ValidDir", processed?.children?.first()?.name)
    }
    
    @Test
    fun testFileTreeProcessor_returnsNullIfRootIsEmptyDirectory() {
        val root = FileNode(
            id = "root",
            name = "root",
            isDirectory = true,
            children = listOf(
                FileNode("1", "image.png", false) 
            )
        )

        val processed = FileTreeProcessor.processTree(root)
        assertNull(processed)
    }

    @Test
    fun testFileTreeProcessor_sortsDirectoriesBeforeFiles() {
        val root = FileNode(
            id = "root",
            name = "root",
            isDirectory = true,
            children = listOf(
                FileNode("1", "z_file.md", false),
                FileNode("2", "a_dir", true, listOf(FileNode("2.1", "doc.md", false))),
                FileNode("3", "a_file.md", false),
                FileNode("4", "z_dir", true, listOf(FileNode("4.1", "doc.md", false)))
            )
        )

        val processed = FileTreeProcessor.processTree(root)
        
        assertEquals(4, processed?.children?.size)
        
        // Directories first, sorted alphabetically
        assertEquals("a_dir", processed?.children?.get(0)?.name)
        assertEquals("z_dir", processed?.children?.get(1)?.name)
        
        // Then files, sorted alphabetically
        assertEquals("a_file.md", processed?.children?.get(2)?.name)
        assertEquals("z_file.md", processed?.children?.get(3)?.name)
    }
}
