package com.fastmd.android.data

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONException
import org.json.JSONObject
import java.io.IOException
import java.net.URLEncoder
import kotlin.coroutines.resume

/**
 * Microsoft Graph backed [OneDriveDataSource]. No Android types — the only
 * collaborator is an [OkHttpClient], which tests can swap for one pointed
 * at a `MockWebServer`.
 */
class GraphOneDriveDataSource(
    private val client: OkHttpClient = OkHttpClient(),
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
) : OneDriveDataSource {

    override suspend fun fetchTree(
        folderPath: String,
        accessToken: String,
    ): OneDriveResult<TreeFetch> = withContext(ioDispatcher) {
        val cleanPath = folderPath.removePrefix("./").removePrefix("/")
        val rootUrl = if (cleanPath.isEmpty()) {
            "https://graph.microsoft.com/v1.0/me/drive/root/children"
        } else {
            val encoded = cleanPath.split("/").joinToString("/") {
                URLEncoder.encode(it, "UTF-8").replace("+", "%20")
            }
            "https://graph.microsoft.com/v1.0/me/drive/root:/$encoded:/children"
        }

        val displayName = cleanPath.ifEmpty { "Root" }
        val root = FileNode(id = "root", name = displayName, isDirectory = true)
        when (val children = fetchChildren(rootUrl, accessToken)) {
            is OneDriveResult.Failure -> OneDriveResult.Failure(children.error)
            is OneDriveResult.Success -> OneDriveResult.Success(
                TreeFetch(
                    root = root.copy(children = children.value.nodes),
                    failedFolders = children.value.failures,
                ),
            )
        }
    }

    override suspend fun fetchFileContent(
        fileId: String,
        accessToken: String,
    ): OneDriveResult<String> = withContext(ioDispatcher) {
        val url = "https://graph.microsoft.com/v1.0/me/drive/items/$fileId/content"
        val request = Request.Builder()
            .url(url)
            .addHeader("Authorization", "Bearer $accessToken")
            .addHeader("Accept", "text/plain, text/markdown, */*;q=0.1")
            .build()

        try {
            client.newCall(request).execute().use { response ->
                if (!response.isSuccessful) {
                    val body = response.body?.string()?.take(512)
                    return@withContext OneDriveResult.Failure(
                        OneDriveError.GraphHttpError(response.code, body),
                    )
                }
                val body = response.body?.string() ?: ""
                OneDriveResult.Success(body)
            }
        } catch (e: IOException) {
            OneDriveResult.Failure(OneDriveError.Unknown(e.message ?: e::class.simpleName ?: "Network error"))
        }
    }

    private data class ChildBatch(
        val nodes: List<FileNode>,
        val failures: List<String>,
    )

    /**
     * Recursively walk a paged folder listing. Failures on individual
     * pages don't abort the whole walk — they're collected so the UI can
     * surface them as a partial-load warning.
     */
    private fun fetchChildren(initialUrl: String, accessToken: String): OneDriveResult<ChildBatch> {
        val nodes = mutableListOf<FileNode>()
        val failures = mutableListOf<String>()
        var url: String? = initialUrl

        while (url != null) {
            val request = Request.Builder()
                .url(url)
                .addHeader("Authorization", "Bearer $accessToken")
                .addHeader("Accept", "application/json")
                .build()

            try {
                client.newCall(request).execute().use { response ->
                    if (!response.isSuccessful) {
                        failures += url
                        url = null
                        return OneDriveResult.Success(ChildBatch(nodes, failures))
                    }
                    val body = response.body?.string() ?: "{}"
                    val json = try {
                        JSONObject(body)
                    } catch (e: JSONException) {
                        failures += url
                        url = null
                        return OneDriveResult.Success(ChildBatch(nodes, failures))
                    }

                    val values = json.optJSONArray("value")
                    if (values != null) {
                        for (i in 0 until values.length()) {
                            val item = values.optJSONObject(i) ?: continue
                            val parsed = parseItem(item) ?: continue
                            if (parsed.isDirectory) {
                                val childUrl =
                                    "https://graph.microsoft.com/v1.0/me/drive/items/${parsed.id}/children"
                                when (val children = fetchChildren(childUrl, accessToken)) {
                                    is OneDriveResult.Failure -> failures += parsed.name
                                    is OneDriveResult.Success -> {
                                        nodes += parsed.copy(children = children.value.nodes)
                                        failures += children.value.failures
                                    }
                                }
                            } else {
                                nodes += parsed
                            }
                        }
                    }

                    url = json.optString("@odata.nextLink", null).takeIf { it.isNotEmpty() }
                }
            } catch (e: IOException) {
                failures += url ?: initialUrl
                url = null
            }
        }
        return OneDriveResult.Success(ChildBatch(nodes, failures))
    }

    private fun parseItem(item: JSONObject): FileNode? {
        val name = item.optString("name").takeIf { it.isNotEmpty() } ?: return null
        val id = item.optString("id").takeIf { it.isNotEmpty() } ?: return null
        val isDir = item.has("folder")
        return FileNode(id = id, name = name, isDirectory = isDir)
    }
}
