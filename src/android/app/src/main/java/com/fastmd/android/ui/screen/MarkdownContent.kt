package com.fastmd.android.ui.screen

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.mikepenz.markdown.m3.Markdown

/**
 * Thin wrapper around mikepenz's M3-themed [Markdown] composable.
 *
 * Keeps the import site stable so swapping renderers later only touches
 * this file.
 */
@Composable
fun MarkdownContent(
    content: String,
    modifier: Modifier = Modifier,
) {
    Markdown(
        content = content,
        modifier = modifier.fillMaxSize(),
    )
}
