package com.fastmd.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private lateinit var oneDriveManager: OneDriveManager

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        oneDriveManager = OneDriveManager(this)

        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    var isAuthenticated by remember { mutableStateOf(false) }
                    var accessToken by remember { mutableStateOf("") }
                    var rootNode by remember { mutableStateOf<FileNode?>(null) }
                    var isLoading by remember { mutableStateOf(false) }
                    var rootFolderInput by remember { mutableStateOf("./Wiki") }
                    var selectedFileContent by remember { mutableStateOf<String?>(null) }
                    var selectedFileName by remember { mutableStateOf<String?>(null) }
                    var errorMessage by remember { mutableStateOf<String?>(null) }
                    val scope = rememberCoroutineScope()

                    LaunchedEffect(Unit) {
                        oneDriveManager.init(
                            onSuccess = { /* Ready to sign in */ },
                            onError = { e -> errorMessage = e.message ?: "Initialization failed" }
                        )
                    }

                    errorMessage?.let { msg ->
                        AlertDialog(
                            onDismissRequest = { errorMessage = null },
                            title = { Text("Error") },
                            text = { Text(msg) },
                            confirmButton = {
                                TextButton(onClick = { errorMessage = null }) { Text("OK") }
                            }
                        )
                    }

                    if (!isAuthenticated) {
                        Column(
                            modifier = Modifier.fillMaxSize(),
                            verticalArrangement = Arrangement.Center,
                            horizontalAlignment = Alignment.CenterHorizontally
                        ) {
                            Text("FastMD Android Viewer", style = MaterialTheme.typography.headlineMedium)
                            Spacer(Modifier.height(16.dp))
                            Button(onClick = {
                                scope.launch {
                                    try {
                                        accessToken = oneDriveManager.signIn()
                                        isAuthenticated = true
                                    } catch (e: Exception) {
                                        errorMessage = e.message ?: "Sign-in failed"
                                        e.printStackTrace()
                                    }
                                }
                            }) {
                                Text("Sign In with OneDrive")
                            }
                        }
                    } else {
                        // Two Pane Layout
                        Row(modifier = Modifier.fillMaxSize()) {
                            // Left Pane: Directory View
                            Column(
                                modifier = Modifier
                                    .weight(1f)
                                    .fillMaxHeight()
                                    .background(Color(0xFF2D2D30))
                                    .padding(8.dp)
                            ) {
                                OutlinedTextField(
                                    value = rootFolderInput,
                                    onValueChange = { rootFolderInput = it },
                                    label = { Text("Root Folder") },
                                    modifier = Modifier.fillMaxWidth()
                                )
                                Spacer(Modifier.height(8.dp))
                                Button(
                                    onClick = {
                                        scope.launch {
                                            isLoading = true
                                            try {
                                                val rawTree = oneDriveManager.fetchFiles(accessToken, rootFolderInput)
                                                rootNode = FileTreeProcessor.processTree(rawTree)
                                            } catch (e: Exception) {
                                                errorMessage = e.message ?: "Failed to load folder"
                                                e.printStackTrace()
                                            } finally {
                                                isLoading = false
                                            }
                                        }
                                    },
                                    modifier = Modifier.fillMaxWidth()
                                ) {
                                    Text("Load Folder")
                                }
                                
                                Spacer(Modifier.height(16.dp))
                                
                                if (isLoading) {
                                    CircularProgressIndicator(modifier = Modifier.align(Alignment.CenterHorizontally))
                                } else {
                                    LazyColumn {
                                        rootNode?.let { node ->
                                            item {
                                                FileTreeView(node = node, depth = 0, onFileClick = { file ->
                                                    scope.launch {
                                                        file.downloadUrl?.let { url ->
                                                            isLoading = true
                                                            try {
                                                                selectedFileContent = oneDriveManager.fetchFileContent(url)
                                                                selectedFileName = file.name
                                                            } catch (e: Exception) {
                                                                errorMessage = e.message ?: "Failed to download file"
                                                                e.printStackTrace()
                                                            } finally {
                                                                isLoading = false
                                                            }
                                                        }
                                                    }
                                                })
                                            }
                                        }
                                    }
                                }
                            }

                            // Right Pane: File Viewer
                            Column(
                                modifier = Modifier
                                    .weight(2f)
                                    .fillMaxHeight()
                                    .padding(16.dp)
                                    .verticalScroll(rememberScrollState())
                            ) {
                                if (selectedFileContent != null) {
                                    Text(
                                        text = selectedFileName ?: "File",
                                        style = MaterialTheme.typography.headlineMedium
                                    )
                                    Spacer(Modifier.height(16.dp))
                                    Text(text = selectedFileContent!!)
                                } else {
                                    Text("Select a markdown file to view", style = MaterialTheme.typography.bodyLarge)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun FileTreeView(node: FileNode, depth: Int, onFileClick: (FileNode) -> Unit) {
    val padding = (depth * 16).dp
    var expanded by remember { mutableStateOf(true) }

    Column(modifier = Modifier.fillMaxWidth().padding(start = padding)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable {
                    if (node.isDirectory) expanded = !expanded
                    else onFileClick(node)
                }
                .padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            val icon = if (node.isDirectory) {
                if (expanded) "📂" else "📁"
            } else "📄"
            Text("$icon ${node.name}", color = Color.White)
        }

        if (node.isDirectory && expanded) {
            for (child in node.children) {
                FileTreeView(node = child, depth = depth + 1, onFileClick = onFileClick)
            }
        }
    }
}
