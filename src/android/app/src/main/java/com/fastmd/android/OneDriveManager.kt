package com.fastmd.android

import android.app.Activity
import android.util.Log
import com.microsoft.identity.client.AuthenticationCallback
import com.microsoft.identity.client.IAuthenticationResult
import com.microsoft.identity.client.IPublicClientApplication
import com.microsoft.identity.client.PublicClientApplication
import com.microsoft.identity.client.exception.MsalException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.io.IOException
import java.net.URLEncoder
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

class OneDriveManager(private val activity: Activity) {
    private var msalApp: IPublicClientApplication? = null
    private val scopes = arrayOf("Files.Read.All")
    private val client = OkHttpClient()

    fun init(onSuccess: () -> Unit, onError: (Exception) -> Unit) {
        PublicClientApplication.createSingleAccountPublicClientApplication(
            activity,
            R.raw.auth_config_single_account,
            object : IPublicClientApplication.ISingleAccountApplicationCreatedListener {
                override fun onCreated(application: com.microsoft.identity.client.ISingleAccountPublicClientApplication?) {
                    msalApp = application
                    onSuccess()
                }

                override fun onError(exception: MsalException?) {
                    exception?.let { onError(it) }
                }
            })
    }

    suspend fun signIn(): String = suspendCancellableCoroutine { cont ->
        msalApp?.acquireToken(activity, scopes, object : AuthenticationCallback {
            override fun onSuccess(authenticationResult: IAuthenticationResult) {
                if (cont.isActive) cont.resume(authenticationResult.accessToken)
            }

            override fun onError(exception: MsalException) {
                if (cont.isActive) cont.resumeWithException(exception)
            }

            override fun onCancel() {
                if (cont.isActive) cont.resumeWithException(Exception("User cancelled sign in"))
            }
        }) ?: run { if (cont.isActive) cont.resumeWithException(Exception("MSAL not initialized")) }
    }

    suspend fun fetchFiles(accessToken: String, folderPath: String = "./Wiki"): FileNode = withContext(Dispatchers.IO) {
        val cleanPath = folderPath.removePrefix("./").removePrefix("/")
        val url = if (cleanPath.isEmpty()) {
            "https://graph.microsoft.com/v1.0/me/drive/root/children"
        } else {
            val encodedPath = cleanPath.split("/").joinToString("/") { URLEncoder.encode(it, "UTF-8").replace("+", "%20") }
            "https://graph.microsoft.com/v1.0/me/drive/root:/$encodedPath:/children"
        }
        
        val rootNode = FileNode("root", cleanPath.ifEmpty { "Root" }, true)
        val children = fetchChildren(url, accessToken)
        rootNode.copy(children = children)
    }

    private fun fetchChildren(initialUrl: String, accessToken: String): List<FileNode> {
        val nodes = mutableListOf<FileNode>()
        var url: String? = initialUrl

        while (url != null) {
            val request = Request.Builder()
                .url(url)
                .addHeader("Authorization", "Bearer $accessToken")
                .build()

            client.newCall(request).execute().use { response ->
                if (!response.isSuccessful) {
                    return nodes // Stop paginating if error occurs
                }

                val jsonResponse = JSONObject(response.body?.string() ?: "{}")
                val values = jsonResponse.optJSONArray("value") ?: return nodes

                for (i in 0 until values.length()) {
                    val item = values.getJSONObject(i)
                    val name = item.getString("name")
                    val id = item.getString("id")
                    val isDir = item.has("folder")
                    val downloadUrl = item.optString("@microsoft.graph.downloadUrl", null)

                    val node = FileNode(id, name, isDir, downloadUrl = downloadUrl)
                    if (isDir) {
                        val childUrl = "https://graph.microsoft.com/v1.0/me/drive/items/$id/children"
                        node.children = fetchChildren(childUrl, accessToken)
                    }
                    nodes.add(node)
                }

                url = jsonResponse.optString("@odata.nextLink", null).takeIf { it.isNotEmpty() }
            }
        }
        return nodes
    }
    
    suspend fun fetchFileContent(downloadUrl: String): String = withContext(Dispatchers.IO) {
        val request = Request.Builder().url(downloadUrl).build()
        client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                throw IOException("Failed to download file: HTTP ${response.code}")
            }
            response.body?.string() ?: ""
        }
    }
}
