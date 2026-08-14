package com.fastmd.android.ui.screen

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.fastmd.android.data.FileNode
import com.fastmd.android.ui.theme.FastMDTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Compose UI tests for [FileBrowserScreen]. Runs on the JVM via
 * Robolectric. Verifies the two-pane layout renders the expected text
 * and that file clicks invoke the callback.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w1024dp-h768dp")
class FileBrowserScreenTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun emptyState_showsHint() {
        composeTestRule.setContent {
            FastMDTheme {
                FileBrowserScreen(
                    rootFolderInput = "./Wiki",
                    onRootFolderChange = { },
                    isLoading = false,
                    isLoadingTree = false,
                    isLoadingFile = false,
                    onLoadFolderClick = { },
                    rootNode = null,
                    folderLoadErrors = emptyList(),
                    onFileClick = { },
                    selectedFileName = null,
                    selectedFileContent = null,
                )
            }
        }
        composeTestRule.onNodeWithText("Root Folder").assertIsDisplayed()
        composeTestRule.onNodeWithText("Load Folder").assertIsDisplayed()
        composeTestRule.onNodeWithText("No folder loaded yet. Tap Load Folder.").assertIsDisplayed()
    }

    @Test
    fun rootNodeWithChildren_rendersFileNames() {
        val tree = FileNode(
            id = "root",
            name = "Wiki",
            isDirectory = true,
            children = listOf(
                FileNode("1", "readme.md", false),
                FileNode("2", "guide.md", false),
            ),
        )
        composeTestRule.setContent {
            FastMDTheme {
                FileBrowserScreen(
                    rootFolderInput = "./Wiki",
                    onRootFolderChange = { },
                    isLoading = false,
                    isLoadingTree = false,
                    isLoadingFile = false,
                    onLoadFolderClick = { },
                    rootNode = tree,
                    folderLoadErrors = emptyList(),
                    onFileClick = { },
                    selectedFileName = null,
                    selectedFileContent = null,
                )
            }
        }
        composeTestRule.onNodeWithText("Wiki").assertIsDisplayed()
        composeTestRule.onNodeWithText("readme.md").assertIsDisplayed()
        composeTestRule.onNodeWithText("guide.md").assertIsDisplayed()
    }

    @Test
    fun fileClick_invokesCallback() {
        val tree = FileNode(
            id = "root",
            name = "Wiki",
            isDirectory = true,
            children = listOf(FileNode("1", "readme.md", false)),
        )
        var clicked: FileNode? = null
        composeTestRule.setContent {
            FastMDTheme {
                FileBrowserScreen(
                    rootFolderInput = "./Wiki",
                    onRootFolderChange = { },
                    isLoading = false,
                    isLoadingTree = false,
                    isLoadingFile = false,
                    onLoadFolderClick = { },
                    rootNode = tree,
                    folderLoadErrors = emptyList(),
                    onFileClick = { clicked = it },
                    selectedFileName = null,
                    selectedFileContent = null,
                )
            }
        }
        composeTestRule.onNodeWithText("readme.md").performClick()
        check(clicked?.name == "readme.md") { "expected readme.md, got $clicked" }
    }

    @Test
    fun partialFailures_showSnackbar() {
        val tree = FileNode("root", "Wiki", true, children = emptyList())
        composeTestRule.setContent {
            FastMDTheme {
                FileBrowserScreen(
                    rootFolderInput = "./Wiki",
                    onRootFolderChange = { },
                    isLoading = false,
                    isLoadingTree = false,
                    isLoadingFile = false,
                    onLoadFolderClick = { },
                    rootNode = tree,
                    folderLoadErrors = listOf("/a", "/b", "/c"),
                    onFileClick = { },
                    selectedFileName = null,
                    selectedFileContent = null,
                )
            }
        }
        composeTestRule.onNodeWithText(
            "3 folder(s) failed to load. Showing what we could.",
        ).assertIsDisplayed()
    }
}
