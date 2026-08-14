package com.fastmd.android.ui.viewmodel

import android.os.Parcelable
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.fastmd.android.data.FileTreeProcessor
import com.fastmd.android.data.OneDriveError
import com.fastmd.android.data.OneDriveResult
import com.fastmd.android.data.OneDriveSource
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.parcelize.Parcelize

/**
 * Holds the [FastMDUiState] and translates UI intents into calls on the
 * injected [OneDriveSource]. State survives configuration changes
 * (because the ViewModel does) and process death (because
 * [SavedStateHandle] persists a serialized snapshot).
 */
class FastMDViewModel(
    private val source: OneDriveSource,
    private val savedStateHandle: SavedStateHandle,
) : ViewModel() {

    private val _uiState: MutableStateFlow<FastMDUiState> = MutableStateFlow(
        savedStateHandle.get<UiStateSnapshot>(KEY_STATE)?.toUiState() ?: FastMDUiState(),
    )
    val uiState: StateFlow<FastMDUiState> = _uiState.asStateFlow()

    init {
        // Persist a slim snapshot on every change so process death is
        // recoverable. We don't persist the full tree or the loaded file
        // body — both are re-fetched cheaply on relaunch.
        viewModelScope.launch {
            _uiState.collect { snapshot ->
                savedStateHandle[KEY_STATE] = UiStateSnapshot.from(snapshot)
            }
        }
        // Kick off initialisation in the background. ViewModelScope is
        // cancelled automatically on ViewModel cleared.
        viewModelScope.launch { initialise() }
    }

    private suspend fun initialise() {
        _uiState.update { it.copy(isInitialising = true, error = null) }
        when (val init = source.init()) {
            is OneDriveResult.Failure -> {
                _uiState.update { it.copy(isInitialising = false, error = init.error) }
                return
            }
            is OneDriveResult.Success -> Unit
        }
        attemptSilentSignIn()
    }

    private suspend fun attemptSilentSignIn() {
        when (val result = source.signInSilently()) {
            is OneDriveResult.Success -> {
                _uiState.update {
                    it.copy(
                        isInitialising = false,
                        authState = AuthState.SignedIn(result.value),
                        error = null,
                    )
                }
            }
            is OneDriveResult.Failure -> {
                _uiState.update {
                    val next = it.copy(isInitialising = false)
                    if (result.error is OneDriveError.NoCachedAccount) {
                        next.copy(authState = AuthState.SignedOut)
                    } else {
                        next.copy(authState = AuthState.SignedOut, error = result.error)
                    }
                }
            }
        }
    }

    fun onSignInClick() {
        if (_uiState.value.isLoading) return
        viewModelScope.launch {
            _uiState.update { it.copy(isInitialising = true, error = null) }
            when (val result = source.signIn()) {
                is OneDriveResult.Success -> {
                    _uiState.update {
                        it.copy(
                            isInitialising = false,
                            authState = AuthState.SignedIn(result.value),
                            error = null,
                        )
                    }
                }
                is OneDriveResult.Failure -> {
                    val isUserCancel = result.error is OneDriveError.Cancelled
                    _uiState.update {
                        it.copy(
                            isInitialising = false,
                            authState = AuthState.SignedOut,
                            error = if (isUserCancel) null else result.error,
                        )
                    }
                }
            }
        }
    }

    fun onRootFolderChange(value: String) {
        _uiState.update { it.copy(rootFolderInput = value) }
    }

    fun onLoadFolderClick() {
        val state = _uiState.value
        if (state.isLoading) return
        viewModelScope.launch {
            _uiState.update {
                it.copy(
                    isLoadingTree = true,
                    folderLoadErrors = emptyList(),
                    error = null,
                    selectedFileContent = null,
                    selectedFileName = null,
                )
            }
            val result = source.fetchTree(state.rootFolderInput)
            _uiState.update { current ->
                when (result) {
                    is OneDriveResult.Success -> {
                        val processed = FileTreeProcessor.processTree(result.value.root)
                        current.copy(
                            isLoadingTree = false,
                            rootNode = processed,
                            folderLoadErrors = result.value.failedFolders,
                        )
                    }
                    is OneDriveResult.Failure -> current.copy(
                        isLoadingTree = false,
                        error = result.error,
                    )
                }
            }
        }
    }

    fun onFileClick(file: com.fastmd.android.data.FileNode) {
        val state = _uiState.value
        if (state.isLoading) return
        viewModelScope.launch {
            _uiState.update {
                it.copy(isLoadingFile = true, selectedFileName = file.name, error = null)
            }
            when (val result = source.fetchFileContent(file.id)) {
                is OneDriveResult.Success -> _uiState.update {
                    it.copy(isLoadingFile = false, selectedFileContent = result.value)
                }
                is OneDriveResult.Failure -> _uiState.update {
                    it.copy(
                        isLoadingFile = false,
                        selectedFileContent = null,
                        error = result.error,
                    )
                }
            }
        }
    }

    fun onErrorDismiss() {
        _uiState.update { it.copy(error = null) }
    }

    /**
     * Slim [Parcelable] snapshot of the bits of [FastMDUiState] worth
     * surviving process death. The file body and the loaded tree are
     * intentionally omitted.
     */
    @Parcelize
    private data class UiStateSnapshot(
        val rootFolderInput: String,
        val selectedFileName: String?,
        val authState: AuthState,
    ) : Parcelable {
        fun toUiState(): FastMDUiState = FastMDUiState(
            isInitialising = false,
            authState = authState,
            rootFolderInput = rootFolderInput,
            selectedFileName = selectedFileName,
        )

        companion object {
            fun from(state: FastMDUiState): UiStateSnapshot = UiStateSnapshot(
                rootFolderInput = state.rootFolderInput,
                selectedFileName = state.selectedFileName,
                authState = state.authState,
            )
        }
    }

    companion object {
        private const val KEY_STATE = "fastmd.ui_state.v1"

        /**
         * Factory for [viewModel()]. Uses [createSavedStateHandle] so the
         * [SavedStateHandle] is wired into the activity / nav-graph
         * scope and survives both configuration changes and process
         * death.
         */
        fun factory(source: OneDriveSource) = viewModelFactory {
            initializer {
                FastMDViewModel(source = source, savedStateHandle = createSavedStateHandle())
            }
        }
    }
}
