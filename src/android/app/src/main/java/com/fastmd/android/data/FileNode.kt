package com.fastmd.android.data

/**
 * One item in the OneDrive tree, recursive.
 *
 * The Rust port of this type lives in `src/android.egui/src/file_node.rs`
 * and the two must stay in lockstep per the egui AGENTS.md. The Kotlin port
 * dropped the short-lived `downloadUrl` field: the stable Graph API endpoint
 * `/me/drive/items/{id}/content` is used for content fetches instead, so a
 * cached pre-authenticated URL is unnecessary.
 */
data class FileNode(
    val id: String,
    val name: String,
    val isDirectory: Boolean,
    val children: List<FileNode> = emptyList(),
)
