package com.fastmd.android.data

import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * [OneDriveSource] implementation that orchestrates the [OneDriveAuth]
 * and [OneDriveDataSource] halves. Holds the access token in memory and
 * transparently re-authenticates if the cached one has been cleared.
 *
 * The token is *not* persisted to disk; silent acquisition is preferred
 * so we get a fresh token from MSAL's secure cache each session.
 */
class MsalOneDriveSource(
    private val auth: OneDriveAuth,
    private val data: OneDriveDataSource,
) : OneDriveSource {

    private val tokenLock = Mutex()
    @Volatile private var cachedToken: String? = null

    override suspend fun init(): OneDriveResult<Unit> = auth.init()

    override suspend fun signInSilently(): OneDriveResult<String> {
        val result = auth.acquireTokenSilent()
        if (result is OneDriveResult.Success) {
            cachedToken = result.value
        }
        return result
    }

    override suspend fun signIn(): OneDriveResult<String> {
        val result = auth.acquireTokenInteractive()
        if (result is OneDriveResult.Success) {
            cachedToken = result.value
        }
        return result
    }

    override suspend fun fetchTree(folderPath: String): OneDriveResult<TreeFetch> {
        val token = ensureToken() ?: return OneDriveResult.Failure(OneDriveError.NotInitialized)
        return data.fetchTree(folderPath, token)
    }

    override suspend fun fetchFileContent(fileId: String): OneDriveResult<String> {
        val token = ensureToken() ?: return OneDriveResult.Failure(OneDriveError.NotInitialized)
        return data.fetchFileContent(fileId, token)
    }

    private suspend fun ensureToken(): String? = tokenLock.withLock {
        cachedToken ?: when (val result = auth.acquireTokenSilent()) {
            is OneDriveResult.Success -> {
                cachedToken = result.value
                result.value
            }
            is OneDriveResult.Failure -> null
        }
    }
}
