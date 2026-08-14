package com.fastmd.android.ui.theme

import androidx.compose.ui.graphics.Color

/**
 * Centralized FastMD palette. Pulled out of [FastMDTheme] so the palette
 * can be referenced from preview composables and other UI files without
 * dragging the whole theme in.
 */
internal object FastMDColors {
    val Background = Color(0xFF1E1E1E)
    val Surface = Color(0xFF2D2D30)
    val OnSurface = Color(0xFFE6E6E6)
    val Primary = Color(0xFF7AA2F7)
    val OnPrimary = Color(0xFF1A1A1A)
    val Secondary = Color(0xFFBB9AF7)
    val OnSecondary = Color(0xFF1A1A1A)
}
