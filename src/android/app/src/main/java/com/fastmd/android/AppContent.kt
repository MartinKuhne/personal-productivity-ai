package com.fastmd.android

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.fastmd.android.ui.screen.AuthScreen
import com.fastmd.android.ui.screen.FileBrowserScreen
import com.fastmd.android.ui.toDisplayMessage
import com.fastmd.android.ui.viewmodel.AuthState
import com.fastmd.android.ui.viewmodel.FastMDViewModel

/**
 * Root composable. Collects state from [FastMDViewModel] and routes
 * between [AuthScreen] and [FileBrowserScreen]. Owns no state of its
 * own — every input is a parameter, every event is a callback.
 */
@Composable
fun AppContent(viewModel: FastMDViewModel) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val genericFailure = stringResource(R.string.error_generic_failure)

    state.error?.let { error ->
        AlertDialog(
            onDismissRequest = viewModel::onErrorDismiss,
            title = { Text(stringResource(R.string.dialog_title_error)) },
            text = { Text(error.toDisplayMessage(genericFailure)) },
            confirmButton = {
                TextButton(onClick = viewModel::onErrorDismiss) {
                    Text(stringResource(R.string.dialog_button_ok))
                }
            },
        )
    }

    when (state.authState) {
        AuthState.Initialising, AuthState.SignedOut -> AuthScreen(
            isInitialising = state.isInitialising,
            error = state.error,
            onSignInClick = viewModel::onSignInClick,
        )
        is AuthState.SignedIn -> FileBrowserScreen(
            rootFolderInput = state.rootFolderInput,
            onRootFolderChange = viewModel::onRootFolderChange,
            isLoading = state.isLoading,
            isLoadingTree = state.isLoadingTree,
            isLoadingFile = state.isLoadingFile,
            onLoadFolderClick = viewModel::onLoadFolderClick,
            rootNode = state.rootNode,
            folderLoadErrors = state.folderLoadErrors,
            onFileClick = viewModel::onFileClick,
            selectedFileName = state.selectedFileName,
            selectedFileContent = state.selectedFileContent,
        )
    }
}
