package com.fastmd.android.ui.screen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.fastmd.android.R
import com.fastmd.android.data.OneDriveError
import com.fastmd.android.ui.toDisplayMessage

/**
 * Pre-authentication landing screen. Centered title + the OneDrive
 * sign-in trigger. The actual auth flow lives in the data layer; this
 * composable just exposes a callback.
 */
@Composable
fun AuthScreen(
    isInitialising: Boolean,
    error: OneDriveError?,
    onSignInClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val genericFailure = stringResource(R.string.error_generic_failure)
    Column(
        modifier = modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = stringResource(R.string.app_title),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.semantics { heading() },
        )
        Spacer(Modifier.height(16.dp))
        if (isInitialising) {
            CircularProgressIndicator()
        } else {
            Button(onClick = onSignInClick) {
                Text(stringResource(R.string.sign_in_with_onedrive))
            }
        }
        error?.let {
            Spacer(Modifier.height(16.dp))
            Text(
                text = it.toDisplayMessage(genericFailure),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
            )
        }
    }
}
