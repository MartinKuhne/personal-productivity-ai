package com.fastmd.android.data

/**
 * The data-only half of OneDrive. No Android types — fully unit-testable
 * with `MockWebServer`.
 */
interface OneDriveDataSource {
    /**
     * Walk the tree rooted at [folderPath] using the given [accessToken]
     * and return the synthetic root node plus the list of folder paths
     * that failed during the recursive walk. A folder failure is not
     * fatal: the rest of the tree is still returned.
     */
    suspend fun fetchTree(folderPath: String, accessToken: String): OneDriveResult<TreeFetch>

    /** Download the body of the file identified by [fileId]. */
    suspend fun fetchFileContent(fileId: String, accessToken: String): OneDriveResult<String>
}
