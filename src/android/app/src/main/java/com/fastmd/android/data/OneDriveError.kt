package com.fastmd.android.data

/**
 * The flat error surface the UI layer sees for every OneDrive operation.
 *
 * Subclasses carry enough information for the UI to decide between a
 * "retry", "sign in", or "reconfigure" action, without leaking platform
 * types (e.g. `MsalException`) into the view layer.
 */
sealed class OneDriveError {

    /**
     * The MSAL client is not initialised yet. Surfaces when a UI flow
     * races ahead of [OneDriveAuth.init] finishing.
     */
    data object NotInitialized : OneDriveError()

    /**
     * Silent token acquisition returned no cached account. UI should fall
     * back to an interactive sign-in.
     */
    data object NoCachedAccount : OneDriveError()

    /** The user explicitly cancelled the interactive sign-in. */
    data object Cancelled : OneDriveError()

    /**
     * The MSAL client was constructed but the `auth_config_single_account.json`
     * is still the placeholder shipped with the repo. UI should direct the
     * user to set up an Azure AD app registration.
     */
    data class Misconfigured(val detail: String) : OneDriveError()

    /** MSAL or authentication SDK reported a non-recoverable error. */
    data class AuthFailed(val detail: String) : OneDriveError()

    /** Graph API returned a non-2xx response. */
    data class GraphHttpError(val httpCode: Int, val bodyExcerpt: String?) : OneDriveError()

    /** Graph API returned malformed JSON we couldn't parse. */
    data class GraphParseError(val detail: String) : OneDriveError()

    /** Caller passed a bad URL (e.g. for a file content fetch). */
    data class InvalidUrl(val url: String) : OneDriveError()

    /** Anything else we don't have a category for. */
    data class Unknown(val detail: String) : OneDriveError()
}
