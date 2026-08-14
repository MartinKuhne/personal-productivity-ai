package com.fastmd.android.ui.screen

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Article
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.fastmd.android.data.FileNode

/**
 * Recursive Material-styled file tree. Tapping a directory toggles its
 * expand state; tapping a file invokes [onFileClick].
 */
@Composable
fun FileTreeView(
    node: FileNode,
    depth: Int,
    onFileClick: (FileNode) -> Unit,
    modifier: Modifier = Modifier,
) {
    val paddingStart = (depth * 16).dp
    var expanded by remember { mutableStateOf(true) }

    Column(modifier = modifier.fillMaxWidth().padding(start = paddingStart)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable {
                    if (node.isDirectory) expanded = !expanded
                    else onFileClick(node)
                }
                .padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val icon = when {
                node.isDirectory && expanded -> Icons.Filled.FolderOpen
                node.isDirectory -> Icons.Filled.Folder
                else -> Icons.Filled.Article
            }
            Icon(imageVector = icon, contentDescription = null)
            Text(
                text = "  ${node.name}",
                modifier = Modifier.padding(start = 4.dp),
            )
        }

        if (node.isDirectory && expanded) {
            for (child in node.children) {
                FileTreeView(node = child, depth = depth + 1, onFileClick = onFileClick)
            }
        }
    }
}
