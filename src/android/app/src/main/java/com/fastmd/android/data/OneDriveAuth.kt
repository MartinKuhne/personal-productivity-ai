package com.fastmd.android.data

/**
 * The auth-only half of OneDrive. Concrete MSAL-backed implementation
 * lives in [MsalOneDriveAuth]; tests substitute a fake.
 */
interface OneDriveAuth {
    /** Initialise the MSAL client. Idempotent. */
    suspend fun init(): OneDriveResult<Unit>

    /**
     * Get a token from the cached account. Returns [OneDriveError.NoCachedAccount]
     * if no cached account exists so callers can fall back to interactive
     * sign-in.
     */
    suspend fun acquireTokenSilent(): OneDriveResult<String>

    /** Prompt the user interactively to sign in. */
    suspend fun acquireTokenInteractive(): OneDriveResult<String>
}
