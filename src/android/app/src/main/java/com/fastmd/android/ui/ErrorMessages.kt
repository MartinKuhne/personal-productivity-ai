package com.fastmd.android.ui

import com.fastmd.android.data.OneDriveError

/**
 * Maps the platform-free [OneDriveError] hierarchy to a localized
 * display string. Lives in the UI layer so the [FastMDViewModel] does
 * not need to depend on Android `R.string.*` (which makes ViewModel
 * unit tests harder).
 */
internal fun OneDriveError.toDisplayMessage(default: String): String = when (this) {
    is OneDriveError.NotInitialized -> "Authentication client is not ready yet. Try again in a moment."
    is OneDriveError.NoCachedAccount -> default
    is OneDriveError.Cancelled -> default
    is OneDriveError.Misconfigured -> detail
    is OneDriveError.AuthFailed -> "Sign-in failed: $detail"
    is OneDriveError.GraphHttpError -> "OneDrive returned HTTP $httpCode."
    is OneDriveError.GraphParseError -> "Couldn't read OneDrive's response: $detail"
    is OneDriveError.InvalidUrl -> "Bad URL: $url"
    is OneDriveError.Unknown -> "Unexpected error: $detail"
}
