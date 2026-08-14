package com.fastmd.android.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable

/**
 * FastMD dark theme — built directly from M3 [darkColorScheme] so we don't
 * need a full dynamic/light counterpart for this viewer-only app.
 */
@Composable
fun FastMDTheme(content: @Composable () -> Unit) {
    val colorScheme = darkColorScheme(
        background = FastMDColors.Background,
        surface = FastMDColors.Surface,
        onSurface = FastMDColors.OnSurface,
        primary = FastMDColors.Primary,
        onPrimary = FastMDColors.OnPrimary,
        secondary = FastMDColors.Secondary,
        onSecondary = FastMDColors.OnSecondary,
    )
    MaterialTheme(colorScheme = colorScheme, content = content)
}
