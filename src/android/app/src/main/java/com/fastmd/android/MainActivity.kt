package com.fastmd.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import com.fastmd.android.data.GraphOneDriveDataSource
import com.fastmd.android.data.MsalOneDriveAuth
import com.fastmd.android.data.MsalOneDriveSource
import com.fastmd.android.data.OneDriveSource
import com.fastmd.android.ui.theme.FastMDTheme
import com.fastmd.android.ui.viewmodel.FastMDViewModel

/**
 * Single-activity host. The [FastMDTheme] + [AppContent] composables own
 * all of the actual UI; this class wires Android lifecycle to the
 * Compose tree and constructs the [OneDriveSource] + [FastMDViewModel]
 * graph.
 */
class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val source: OneDriveSource = MsalOneDriveSource(
            auth = MsalOneDriveAuth(this),
            data = GraphOneDriveDataSource(),
        )
        setContent {
            FastMDTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    val viewModel: FastMDViewModel = viewModel(
                        factory = FastMDViewModel.factory(source),
                    )
                    AppContent(viewModel = viewModel)
                }
            }
        }
    }
}
