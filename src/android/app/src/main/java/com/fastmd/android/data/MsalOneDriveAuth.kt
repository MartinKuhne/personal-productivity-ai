package com.fastmd.android.data

import android.app.Activity
import com.fastmd.android.R
import com.microsoft.identity.client.AcquireTokenParameters
import com.microsoft.identity.client.AuthenticationCallback
import com.microsoft.identity.client.IAuthenticationResult
import com.microsoft.identity.client.IPublicClientApplication
import com.microsoft.identity.client.ISingleAccountPublicClientApplication
import com.microsoft.identity.client.PublicClientApplication
import com.microsoft.identity.client.SilentAuthenticationCallback
import com.microsoft.identity.client.exception.MsalClientException
import com.microsoft.identity.client.exception.MsalException
import com.microsoft.identity.client.exception.MsalUiRequiredException
import kotlinx.coroutines.suspendCancellableCoroutine
import org.json.JSONObject
import kotlin.coroutines.resume

/**
 * MSAL-backed [OneDriveAuth] implementation. Owns the
 * [ISingleAccountPublicClientApplication] lifecycle and the configured
 * scopes.
 *
 * The auth_config JSON is validated up front so the user sees a clear
 * "configure your Azure AD app registration" message rather than a
 * confusing MSAL native crash when the placeholder client_id is shipped.
 */
class MsalOneDriveAuth(
    private val activity: Activity,
    private val configResourceId: Int = R.raw.auth_config_single_account,
) : OneDriveAuth {

    private val scopes = arrayOf("Files.Read.All")
    private var msalApp: ISingleAccountPublicClientApplication? = null

    override suspend fun init(): OneDriveResult<Unit> {
        // Validate the bundled config before handing it to MSAL.
        val configError = validateConfig()
        if (configError != null) {
            return OneDriveResult.Failure(OneDriveError.Misconfigured(configError))
        }

        msalApp?.let { return OneDriveResult.Success(Unit) }

        return suspendCancellableCoroutine { cont ->
            PublicClientApplication.createSingleAccountPublicClientApplication(
                activity,
                configResourceId,
                object : IPublicClientApplication.ISingleAccountApplicationCreatedListener {
                    override fun onCreated(application: ISingleAccountPublicClientApplication?) {
                        if (application == null) {
                            if (cont.isActive) {
                                cont.resume(
                                    OneDriveResult.Failure(
                                        OneDriveError.AuthFailed("MSAL returned a null client"),
                                    ),
                                )
                            }
                        } else {
                            msalApp = application
                            if (cont.isActive) cont.resume(OneDriveResult.Success(Unit))
                        }
                    }

                    override fun onError(exception: MsalException?) {
                        if (cont.isActive) {
                            cont.resume(
                                OneDriveResult.Failure(
                                    OneDriveError.AuthFailed(exception?.message ?: "MSAL init failed"),
                                ),
                            )
                        }
                    }
                },
            )
        }
    }

    override suspend fun acquireTokenSilent(): OneDriveResult<String> {
        val app = msalApp ?: return OneDriveResult.Failure(OneDriveError.NotInitialized)
        return suspendCancellableCoroutine { cont ->
            val params = AcquireTokenParameters.Builder()
                .withScopes(scopes.toList())
                .withCallback(object : SilentAuthenticationCallback {
                    override fun onSuccess(result: IAuthenticationResult) {
                        if (cont.isActive) cont.resume(OneDriveResult.Success(result.accessToken))
                    }

                    override fun onError(exception: MsalException) {
                        if (!cont.isActive) return
                        val err = when (exception) {
                            is MsalUiRequiredException -> OneDriveError.NoCachedAccount
                            is MsalClientException -> OneDriveError.Misconfigured(exception.message ?: "MSAL client misconfigured")
                            else -> OneDriveError.AuthFailed(exception.message ?: "Silent auth failed")
                        }
                        cont.resume(OneDriveResult.Failure(err))
                    }
                })
                .build()
            app.acquireTokenSilentAsync(params)
        }
    }

    override suspend fun acquireTokenInteractive(): OneDriveResult<String> {
        val app = msalApp ?: return OneDriveResult.Failure(OneDriveError.NotInitialized)
        return suspendCancellableCoroutine { cont ->
            val params = AcquireTokenParameters.Builder()
                .withActivity(activity)
                .withScopes(scopes.toList())
                .withCallback(object : AuthenticationCallback {
                    override fun onSuccess(result: IAuthenticationResult) {
                        if (cont.isActive) cont.resume(OneDriveResult.Success(result.accessToken))
                    }

                    override fun onError(exception: MsalException) {
                        if (cont.isActive) {
                            cont.resume(
                                OneDriveResult.Failure(
                                    OneDriveError.AuthFailed(exception.message ?: "Interactive auth failed"),
                                ),
                            )
                        }
                    }

                    override fun onCancel() {
                        if (cont.isActive) cont.resume(OneDriveResult.Failure(OneDriveError.Cancelled))
                    }
                })
                .build()
            app.acquireToken(params)
        }
    }

    /**
     * The shipped `auth_config_single_account.json` has
     * `"client_id": "YOUR_CLIENT_ID_HERE"` and a placeholder signature hash
     * in the redirect URI. Catch that before MSAL does, and return a
     * human-friendly [OneDriveError.Misconfigured].
     */
    private fun validateConfig(): String? {
        return try {
            val stream = activity.resources.openRawResource(configResourceId)
            val text = stream.bufferedReader().use { it.readText() }
            val json = JSONObject(text)
            val clientId = json.optString("client_id", "")
            val redirectUri = json.optString("redirect_uri", "")
            when {
                clientId.isBlank() || clientId.startsWith("YOUR_") ->
                    "MSAL client_id is not configured. " +
                        "Edit app/src/main/res/raw/auth_config_single_account.json with your " +
                        "Azure AD app registration's client id."
                redirectUri.contains("signature_hash_here") ->
                    "MSAL redirect_uri contains a placeholder signature hash. " +
                        "Update it to match the SHA-1 of your signing key."
                else -> null
            }
        } catch (t: Throwable) {
            "Could not read auth_config_single_account.json: ${t.message ?: t::class.simpleName}"
        }
    }
}
