package com.fastmd.android.data

import kotlinx.coroutines.test.runTest
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Tests the Graph HTTP layer in isolation. We point the [OkHttpClient]
 * inside [GraphOneDriveDataSource] at a [MockWebServer] so no real
 * network is involved and we can assert exactly which URLs are hit and
 * with which bodies.
 */
class GraphOneDriveDataSourceTest {

    private lateinit var server: MockWebServer
    private lateinit var dataSource: GraphOneDriveDataSource

    @Before
    fun setUp() {
        server = MockWebServer().apply { start() }
        val client = OkHttpClient.Builder().build()
        dataSource = GraphOneDriveDataSource(
            client = client,
            ioDispatcher = kotlinx.coroutines.Dispatchers.Unconfined,
        )
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    @Test
    fun `fetchTree loads root children`() = runTest {
        server.enqueueGraphChildren(
            body = """
                {
                  "value": [
                    { "id": "1", "name": "readme.md" },
                    { "id": "2", "name": "image.png" },
                    { "id": "3", "name": "sub", "folder": {} }
                  ]
                }
            """.trimIndent(),
        )
        // Recursion into the "sub" folder.
        server.enqueueGraphChildren(
            body = """
                { "value": [ { "id": "3.1", "name": "inner.md" } ] }
            """.trimIndent(),
        )

        val result = dataSource.fetchTree(folderPath = "./Wiki", accessToken = "t0k3n")

        val tree = (result as OneDriveResult.Success).value
        assertEquals(2, tree.root.children.size)
        // Non-markdown file is still in the raw tree — filtering happens in
        // FileTreeProcessor, not the data source.
        val sub = tree.root.children.first { it.name == "sub" }
        assertEquals(1, sub.children.size)
        assertEquals("inner.md", sub.children[0].name)
        assertTrue("no failures expected", tree.failedFolders.isEmpty())

        val recorded = server.takeRequest()
        assertEquals("Bearer t0k3n", recorded.getHeader("Authorization"))
    }

    @Test
    fun `fetchTree follows odata nextLink pagination`() = runTest {
        server.enqueueGraphChildren(
            body = """
                {
                  "value": [ { "id": "1", "name": "a.md" } ],
                  "@odata.nextLink": "${server.url("/next")}"
                }
            """.trimIndent(),
        )
        server.enqueueGraphChildren(
            body = """
                { "value": [ { "id": "2", "name": "b.md" } ] }
            """.trimIndent(),
        )

        val result = dataSource.fetchTree(folderPath = "", accessToken = "t0k3n")

        val tree = (result as OneDriveResult.Success).value
        assertEquals(listOf("a.md", "b.md"), tree.root.children.map { it.name })
        assertEquals(2, server.requestCount)
    }

    @Test
    fun `fetchTree collects partial failures and continues`() = runTest {
        // First folder works.
        server.enqueueGraphChildren(
            body = """
                { "value": [ { "id": "1", "name": "ok.md" } ] }
            """.trimIndent(),
        )
        // Second folder blows up with 500.
        server.enqueue(MockResponse().setResponseCode(500).setBody("boom"))

        val result = dataSource.fetchTree(folderPath = "", accessToken = "t0k3n")

        val tree = (result as OneDriveResult.Success).value
        assertEquals(1, tree.root.children.size)
        assertEquals("ok.md", tree.root.children[0].name)
        assertEquals(1, tree.failedFolders.size)
    }

    @Test
    fun `fetchTree returns failure when Graph 4xx is the root call`() = runTest {
        server.enqueue(MockResponse().setResponseCode(401).setBody("unauthorized"))

        val result = dataSource.fetchTree(folderPath = "", accessToken = "bad")

        assertTrue(result is OneDriveResult.Failure)
        val err = (result as OneDriveResult.Failure).error
        assertTrue(err is OneDriveError.GraphHttpError)
        assertEquals(401, (err as OneDriveError.GraphHttpError).httpCode)
    }

    @Test
    fun `fetchTree returns empty root when Graph returns empty value`() = runTest {
        server.enqueueGraphChildren(body = """ { "value": [] } """)

        val result = dataSource.fetchTree(folderPath = "", accessToken = "t0k3n")

        val tree = (result as OneDriveResult.Success).value
        assertTrue(tree.root.children.isEmpty())
    }

    @Test
    fun `fetchFileContent returns body on success`() = runTest {
        server.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/markdown")
                .setBody("# Hello\nWorld"),
        )

        val result = dataSource.fetchFileContent(fileId = "abc", accessToken = "t0k3n")

        assertTrue(result is OneDriveResult.Success)
        assertEquals("# Hello\nWorld", (result as OneDriveResult.Success).value)
        val recorded = server.takeRequest()
        // We should have hit the stable /me/drive/items/{id}/content endpoint.
        assertTrue(
            "path was: ${recorded.path}",
            recorded.path?.endsWith("/me/drive/items/abc/content") == true,
        )
    }

    @Test
    fun `fetchFileContent returns GraphHttpError on non-2xx`() = runTest {
        server.enqueue(MockResponse().setResponseCode(404))

        val result = dataSource.fetchFileContent(fileId = "missing", accessToken = "t0k3n")

        assertTrue(result is OneDriveResult.Failure)
        val err = (result as OneDriveResult.Failure).error
        assertTrue(err is OneDriveError.GraphHttpError)
        assertEquals(404, (err as OneDriveError.GraphHttpError).httpCode)
    }

    /**
     * Enqueue a successful Graph children response at the given relative
     * path. Default path `/me/drive/root/children` covers the root call.
     */
    private fun MockWebServer.enqueueGraphChildren(
        body: String,
        path: String = "/me/drive/root/children",
    ) {
        enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "application/json")
                .setBody(body),
        )
        // Note: MockWebServer doesn't filter by path, so callers enqueue
        // in the order requests come in. This helper is purely sugar so
        // the body is more visible at the call site.
        @Suppress("UNUSED_VARIABLE")
        val unused = path
    }
}
