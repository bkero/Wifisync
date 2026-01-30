package com.wifisync.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material.icons.filled.Wifi
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.wifisync.android.ui.theme.WifisyncTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            WifisyncTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    WifisyncApp()
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WifisyncApp() {
    var credentials by remember { mutableStateOf<List<CredentialSummary>>(emptyList()) }
    var isLoading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }

    // Load credentials on first composition
    LaunchedEffect(Unit) {
        loadCredentials(
            onSuccess = { creds ->
                credentials = creds
                isLoading = false
            },
            onError = { e ->
                error = e
                isLoading = false
            }
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Wifisync") },
                actions = {
                    IconButton(onClick = {
                        isLoading = true
                        loadCredentials(
                            onSuccess = { creds ->
                                credentials = creds
                                isLoading = false
                                error = null
                            },
                            onError = { e ->
                                error = e
                                isLoading = false
                            }
                        )
                    }) {
                        Icon(Icons.Default.Refresh, contentDescription = "Refresh")
                    }
                }
            )
        }
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            when {
                isLoading -> {
                    CircularProgressIndicator(
                        modifier = Modifier.align(Alignment.Center)
                    )
                }
                error != null -> {
                    ErrorContent(
                        message = error!!,
                        modifier = Modifier.align(Alignment.Center)
                    )
                }
                credentials.isEmpty() -> {
                    EmptyContent(
                        modifier = Modifier.align(Alignment.Center)
                    )
                }
                else -> {
                    CredentialsList(
                        credentials = credentials,
                        modifier = Modifier.fillMaxSize()
                    )
                }
            }
        }
    }
}

@Composable
fun CredentialsList(
    credentials: List<CredentialSummary>,
    modifier: Modifier = Modifier
) {
    LazyColumn(
        modifier = modifier,
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        items(credentials) { credential ->
            CredentialCard(credential = credential)
        }
    }
}

@Composable
fun CredentialCard(credential: CredentialSummary) {
    Card(
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Icon(
                imageVector = Icons.Default.Wifi,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(40.dp)
            )

            Spacer(modifier = Modifier.width(16.dp))

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = credential.ssid,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold
                )
                Text(
                    text = formatSecurityType(credential.securityType),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                if (credential.tags.isNotEmpty()) {
                    Text(
                        text = credential.tags.joinToString(", "),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.tertiary
                    )
                }
            }

            if (credential.managed) {
                Icon(
                    imageVector = Icons.Default.Lock,
                    contentDescription = "Managed",
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(24.dp)
                )
            }
        }
    }
}

@Composable
fun EmptyContent(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Icon(
            imageVector = Icons.Default.Wifi,
            contentDescription = null,
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = "No credentials found",
            style = MaterialTheme.typography.titleMedium
        )
        Text(
            text = "Import credentials or add them manually",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

@Composable
fun ErrorContent(message: String, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Icon(
            imageVector = Icons.Default.Warning,
            contentDescription = null,
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.error
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = "Error",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.error
        )
        Text(
            text = message,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

private fun loadCredentials(
    onSuccess: (List<CredentialSummary>) -> Unit,
    onError: (String) -> Unit
) {
    WifisyncCore.listCredentials().fold(
        onSuccess = { credentials ->
            onSuccess(credentials)
        },
        onFailure = { exception ->
            onError(exception.message ?: "Unknown error")
        }
    )
}

private fun formatSecurityType(type: String): String {
    return when (type) {
        "Wpa2Psk" -> "WPA2 Personal"
        "Wpa3Psk" -> "WPA3 Personal"
        "WpaWpa2Psk" -> "WPA/WPA2 Personal"
        "Wpa2Wpa3Psk" -> "WPA2/WPA3 Personal"
        "Open" -> "Open (No Password)"
        "Wpa2Enterprise" -> "WPA2 Enterprise"
        "Wpa3Enterprise" -> "WPA3 Enterprise"
        else -> type
    }
}
