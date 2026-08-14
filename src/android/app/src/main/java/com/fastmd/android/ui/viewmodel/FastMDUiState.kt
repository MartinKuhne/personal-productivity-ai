package com.fastmd.android.ui.viewmodel

import android.os.Parcelable
import com.fastmd.android.data.FileNode
import com.fastmd.android.data.OneDriveError
import kotlinx.parcelize.Parcelize

/**
 * Single source of truth for what the UI shows. The view layer maps
 * [error] (a structured [OneDriveError]) to a localized string at the
 * very last moment so the [FastMDViewModel] stays Android-resource-free.
 */
@Parcelize
data class FastMDUiState(
    val isInitialising: Boolean = true,
    val authState: AuthState = AuthState.SignedOut,
    val rootFolderInput: String = DEFAULT_ROOT_FOLDER,
    val rootNode: FileNode? = null,
    val selectedFileName: String? = null,
    val selectedFileContent: String? = null,
    val isLoadingTree: Boolean = false,
    val isLoadingFile: Boolean = false,
    val folderLoadErrors: List<String> = emptyList(),
    val error: OneDriveError? = null,
) : Parcelable {
    val isLoading: Boolean get() = isLoadingTree || isLoadingFile

    companion object {
        const val DEFAULT_ROOT_FOLDER: String = "./Wiki"
    }
}

sealed class AuthState : Parcelable {
    @Parcelize
    data object Initialising : AuthState()

    @Parcelize
    data object SignedOut : AuthState()

    @Parcelize
    data class SignedIn(val accessToken: String) : AuthState()
}
