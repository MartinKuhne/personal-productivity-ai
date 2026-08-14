package com.fastmd.android.ui.screen

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.fastmd.android.R
import com.fastmd.android.data.FileNode
import com.fastmd.android.ui.theme.FastMDColors

/**
 * Two-pane browser: directory tree on the left, file viewer on the right.
 * Stateless from the parent's perspective — every input is a parameter,
 * every event is a callback. This keeps the screen easy to preview and
 * to drive from a ViewModel.
 */
@Composable
fun FileBrowserScreen(
    rootFolderInput: String,
    onRootFolderChange: (String) -> Unit,
    isLoading: Boolean,
    isLoadingTree: Boolean,
    isLoadingFile: Boolean,
    onLoadFolderClick: () -> Unit,
    rootNode: FileNode?,
    folderLoadErrors: List<String>,
    onFileClick: (FileNode) -> Unit,
    selectedFileName: String?,
    selectedFileContent: String?,
    modifier: Modifier = Modifier,
) {
    val snackbarHost = remember { SnackbarHostState() }
    val failedFoldersMessage = stringResource(
        R.string.folder_load_errors_snackbar,
        folderLoadErrors.size,
    )

    LaunchedEffect(folderLoadErrors) {
        if (folderLoadErrors.isNotEmpty()) {
            snackbarHost.showSnackbar(message = failedFoldersMessage, withDismissAction = true)
        }
    }

    Box(modifier = modifier.fillMaxSize()) {
        Row(modifier = Modifier.fillMaxSize()) {
            DirectoryPane(
                rootFolderInput = rootFolderInput,
                onRootFolderChange = onRootFolderChange,
                isLoading = isLoadingTree,
                onLoadFolderClick = onLoadFolderClick,
                rootNode = rootNode,
                onFileClick = onFileClick,
                modifier = Modifier.weight(1f).fillMaxHeight(),
            )
            FileViewerPane(
                fileName = selectedFileName,
                content = selectedFileContent,
                isLoading = isLoadingFile,
                modifier = Modifier.weight(2f).fillMaxHeight(),
            )
        }
        SnackbarHost(
            hostState = snackbarHost,
            modifier = Modifier.align(Alignment.BottomCenter),
        ) { data -> Snackbar(snackbarData = data) }
    }
}

@Composable
private fun DirectoryPane(
    rootFolderInput: String,
    onRootFolderChange: (String) -> Unit,
    isLoading: Boolean,
    onLoadFolderClick: () -> Unit,
    rootNode: FileNode?,
    onFileClick: (FileNode) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .background(FastMDColors.Surface)
            .padding(8.dp),
    ) {
        OutlinedTextField(
            value = rootFolderInput,
            onValueChange = onRootFolderChange,
            label = { Text(stringResource(R.string.root_folder_label)) },
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(8.dp))
        Button(
            onClick = onLoadFolderClick,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(stringResource(R.string.load_folder))
        }

        Spacer(Modifier.height(16.dp))

        if (isLoading) {
            CircularProgressIndicator(modifier = Modifier.align(Alignment.CenterHorizontally))
        } else if (rootNode == null) {
            Text(
                text = stringResource(R.string.folder_empty_hint),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.align(Alignment.CenterHorizontally),
            )
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.Top,
            ) {
                item {
                    Text(
                        text = rootNode.name,
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 4.dp)
                            .semantics { heading() },
                    )
                }
                items(items = rootNode.children, key = { it.id }) { child ->
                    FileTreeView(node = child, depth = 0, onFileClick = onFileClick)
                }
            }
        }
    }
}
