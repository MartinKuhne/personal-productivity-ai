package com.fastmd.android.ui.viewmodel

import androidx.lifecycle.SavedStateHandle
import com.fastmd.android.data.FileNode
import com.fastmd.android.data.OneDriveError
import com.fastmd.android.data.OneDriveResult
import com.fastmd.android.data.OneDriveSource
import com.fastmd.android.data.TreeFetch
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class FastMDViewModelTest {

    private val dispatcher = StandardTestDispatcher()

    @Before
    fun setUp() {
        Dispatchers.setMain(dispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `init with silent sign-in success transitions to SignedIn`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply { nextSilent = OneDriveResult.Success("t0k3n") }
        val vm = FastMDViewModel(source, SavedStateHandle())

        advanceUntilIdle()

        val state = vm.uiState.value
        assertTrue("isInitialising should be false", !state.isInitialising)
        assertTrue(state.authState is AuthState.SignedIn)
        assertEquals("t0k3n", (state.authState as AuthState.SignedIn).accessToken)
        assertNull(state.error)
    }

    @Test
    fun `init with no cached account stays SignedOut without error`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply { nextSilent = OneDriveResult.Failure(OneDriveError.NoCachedAccount) }
        val vm = FastMDViewModel(source, SavedStateHandle())

        advanceUntilIdle()

        val state = vm.uiState.value
        assertTrue(state.authState is AuthState.SignedOut)
        assertNull(state.error)
    }

    @Test
    fun `init surfaces a Misconfigured error when init fails`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply { nextInit = OneDriveResult.Failure(OneDriveError.Misconfigured("client id is YOUR_CLIENT_ID_HERE")) }
        val vm = FastMDViewModel(source, SavedStateHandle())

        advanceUntilIdle()

        val state = vm.uiState.value
        assertTrue(state.error is OneDriveError.Misconfigured)
    }

    @Test
    fun `interactive sign in transitions to SignedIn on success`() = runTest(dispatcher) {
        val source = FakeOneDriveSource()
        val vm = FastMDViewModel(source, SavedStateHandle())
        advanceUntilIdle()
        source.nextInteractive = OneDriveResult.Success("interactive-token")

        vm.onSignInClick()
        advanceUntilIdle()

        assertTrue(vm.uiState.value.authState is AuthState.SignedIn)
    }

    @Test
    fun `interactive sign in cancellation clears any prior error`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply { nextSilent = OneDriveResult.Failure(OneDriveError.NoCachedAccount) }
        val vm = FastMDViewModel(source, SavedStateHandle())
        advanceUntilIdle()
        source.nextInteractive = OneDriveResult.Failure(OneDriveError.Cancelled)

        vm.onSignInClick()
        advanceUntilIdle()

        val state = vm.uiState.value
        assertTrue(state.authState is AuthState.SignedOut)
        assertNull("user cancellation should not surface an error", state.error)
    }

    @Test
    fun `load folder success sets root node and clears selection`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply { nextSilent = OneDriveResult.Success("t0k3n") }
        val vm = FastMDViewModel(source, SavedStateHandle())
        advanceUntilIdle()

        val tree = FileNode(
            id = "root",
            name = "Root",
            isDirectory = true,
            children = listOf(FileNode("1", "a.md", false)),
        )
        source.nextFetch = OneDriveResult.Success(TreeFetch(root = tree, failedFolders = emptyList()))

        vm.onLoadFolderClick()
        advanceUntilIdle()

        val state = vm.uiState.value
        assertNotNull(state.rootNode)
        assertEquals(1, state.rootNode!!.children.size)
        assertTrue(state.folderLoadErrors.isEmpty())
        assertNull(state.selectedFileContent)
    }

    @Test
    fun `load folder failure surfaces error`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply {
            nextSilent = OneDriveResult.Success("t0k3n")
            nextFetch = OneDriveResult.Failure(OneDriveError.GraphHttpError(500, "boom"))
        }
        val vm = FastMDViewModel(source, SavedStateHandle())
        advanceUntilIdle()

        vm.onLoadFolderClick()
        advanceUntilIdle()

        assertTrue(vm.uiState.value.error is OneDriveError.GraphHttpError)
    }

    @Test
    fun `load folder partial failure populates folderLoadErrors`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply { nextSilent = OneDriveResult.Success("t0k3n") }
        val vm = FastMDViewModel(source, SavedStateHandle())
        advanceUntilIdle()

        val tree = FileNode(id = "root", name = "Root", isDirectory = true, children = emptyList())
        source.nextFetch = OneDriveResult.Success(
            TreeFetch(root = tree, failedFolders = listOf("/Wiki/sub1", "/Wiki/sub2")),
        )

        vm.onLoadFolderClick()
        advanceUntilIdle()

        assertEquals(2, vm.uiState.value.folderLoadErrors.size)
    }

    @Test
    fun `file click loads file content`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply {
            nextSilent = OneDriveResult.Success("t0k3n")
            nextFileContent = OneDriveResult.Success("# Title\nbody")
        }
        val vm = FastMDViewModel(source, SavedStateHandle())
        advanceUntilIdle()

        vm.onFileClick(FileNode("1", "title.md", false))
        advanceUntilIdle()

        val state = vm.uiState.value
        assertEquals("title.md", state.selectedFileName)
        assertEquals("# Title\nbody", state.selectedFileContent)
    }

    @Test
    fun `error dismiss clears the error`() = runTest(dispatcher) {
        val source = FakeOneDriveSource().apply {
            nextInit = OneDriveResult.Failure(OneDriveError.Misconfigured("oops"))
        }
        val vm = FastMDViewModel(source, SavedStateHandle())
        advanceUntilIdle()
        assertNotNull(vm.uiState.value.error)

        vm.onErrorDismiss()
        advanceUntilIdle()

        assertNull(vm.uiState.value.error)
    }

    private class FakeOneDriveSource : OneDriveSource {
        var nextInit: OneDriveResult<Unit> = OneDriveResult.Success(Unit)
        var nextSilent: OneDriveResult<String> = OneDriveResult.Failure(OneDriveError.NoCachedAccount)
        var nextInteractive: OneDriveResult<String> = OneDriveResult.Success("t0k3n")
        var nextFetch: OneDriveResult<TreeFetch> = OneDriveResult.Success(
            TreeFetch(root = FileNode("root", "Root", true, emptyList()), failedFolders = emptyList()),
        )
        var nextFileContent: OneDriveResult<String> = OneDriveResult.Success("")

        var lastFetchedFolder: String? = null
        var lastFetchedFileId: String? = null

        override suspend fun init(): OneDriveResult<Unit> = nextInit
        override suspend fun signInSilently(): OneDriveResult<String> = nextSilent
        override suspend fun signIn(): OneDriveResult<String> = nextInteractive

        override suspend fun fetchTree(folderPath: String): OneDriveResult<TreeFetch> {
            lastFetchedFolder = folderPath
            return nextFetch
        }

        override suspend fun fetchFileContent(fileId: String): OneDriveResult<String> {
            lastFetchedFileId = fileId
            return nextFileContent
        }
    }
}
