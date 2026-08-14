package com.fastmd.android.ui.screen

import androidx.compose.ui.test.assertDoesNotExist
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.fastmd.android.data.OneDriveError
import com.fastmd.android.ui.theme.FastMDTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Compose UI tests for [AuthScreen]. Runs on the JVM via Robolectric.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w360dp-h640dp")
class AuthScreenTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun signInButton_invokesCallback() {
        var clicked = 0
        composeTestRule.setContent {
            FastMDTheme {
                AuthScreen(
                    isInitialising = false,
                    error = null,
                    onSignInClick = { clicked++ },
                )
            }
        }
        composeTestRule.onNodeWithText("Sign In with OneDrive").assertIsDisplayed().performClick()
        check(clicked == 1) { "expected 1 click, got $clicked" }
    }

    @Test
    fun initialisingState_hidesSignInButton() {
        composeTestRule.setContent {
            FastMDTheme {
                AuthScreen(
                    isInitialising = true,
                    error = null,
                    onSignInClick = { },
                )
            }
        }
        composeTestRule.onNodeWithText("Sign In with OneDrive").assertDoesNotExist()
    }

    @Test
    fun error_isDisplayedWhenProvided() {
        composeTestRule.setContent {
            FastMDTheme {
                AuthScreen(
                    isInitialising = false,
                    error = OneDriveError.Misconfigured("client_id is YOUR_CLIENT_ID_HERE"),
                    onSignInClick = { },
                )
            }
        }
        composeTestRule.onNodeWithText(
            "client_id is YOUR_CLIENT_ID_HERE",
            substring = true,
        ).assertIsDisplayed()
    }
}
