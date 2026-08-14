package com.fastmd.android.data

/**
 * Public OneDrive surface the UI / ViewModel talk to. The interface is
 * Android-free so unit tests can substitute a fake without standing up an
 * `Activity` or `Context`.
 */
interface OneDriveSource {
    /** Initialise the underlying auth client. Idempotent. */
    suspend fun init(): OneDriveResult<Unit>

    /**
     * Try to obtain an access token from the cached account without
     * prompting the user. Returns [OneDriveError.NoCachedAccount] if the
     * user has never signed in on this device.
     */
    suspend fun signInSilently(): OneDriveResult<String>

    /** Prompt the user interactively to sign in. */
    suspend fun signIn(): OneDriveResult<String>

    /**
     * Load the full (unprocessed) tree under [folderPath] and return the
     * processed tree plus the list of folder paths that failed during the
     * recursive walk.
     */
    suspend fun fetchTree(folderPath: String): OneDriveResult<TreeFetch>

    /** Download the body of the file identified by [fileId]. */
    suspend fun fetchFileContent(fileId: String): OneDriveResult<String>
}

/** A typed result wrapper so the UI can branch on success / failure. */
sealed class OneDriveResult<out T> {
    data class Success<T>(val value: T) : OneDriveResult<T>()
    data class Failure(val error: OneDriveError) : OneDriveResult<Nothing>()

    inline fun <R> fold(onSuccess: (T) -> R, onFailure: (OneDriveError) -> R): R = when (this) {
        is Success -> onSuccess(value)
        is Failure -> onFailure(error)
    }
}

/** Result of a recursive tree fetch. */
data class TreeFetch(
    val root: FileNode,
    val failedFolders: List<String>,
)
