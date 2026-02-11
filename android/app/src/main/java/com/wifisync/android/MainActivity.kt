package com.wifisync.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.wifisync.android.BuildConfig
import com.wifisync.android.ui.theme.WifisyncTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

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

// Navigation routes
sealed class Screen(val route: String, val title: String, val icon: @Composable () -> Unit) {
    object Credentials : Screen("credentials", "Networks", { Icon(Icons.Default.Wifi, contentDescription = null) })
    object Sync : Screen("sync", "Sync", { Icon(Icons.Default.Sync, contentDescription = null) })
    object Settings : Screen("settings", "Settings", { Icon(Icons.Default.Settings, contentDescription = null) })
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WifisyncApp() {
    val navController = rememberNavController()
    val screens = listOf(Screen.Credentials, Screen.Sync, Screen.Settings)
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = navBackStackEntry?.destination?.route

    Scaffold(
        bottomBar = {
            NavigationBar {
                screens.forEach { screen ->
                    NavigationBarItem(
                        icon = screen.icon,
                        label = { Text(screen.title) },
                        selected = currentRoute == screen.route,
                        onClick = {
                            navController.navigate(screen.route) {
                                popUpTo(navController.graph.startDestinationId) {
                                    saveState = true
                                }
                                launchSingleTop = true
                                restoreState = true
                            }
                        }
                    )
                }
            }
        }
    ) { padding ->
        NavHost(
            navController = navController,
            startDestination = Screen.Credentials.route,
            modifier = Modifier.padding(padding)
        ) {
            composable(Screen.Credentials.route) { CredentialsScreen() }
            composable(Screen.Sync.route) { SyncScreen() }
            composable(Screen.Settings.route) { SettingsScreen() }
        }
    }
}

// ============================================================================
// Credentials Screen
// ============================================================================

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CredentialsScreen() {
    var credentials by remember { mutableStateOf<List<CredentialSummary>>(emptyList()) }
    var isLoading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    fun loadData() {
        scope.launch {
            isLoading = true
            error = null
            withContext(Dispatchers.IO) {
                WifisyncCore.listCredentials()
            }.fold(
                onSuccess = { creds ->
                    credentials = creds
                    isLoading = false
                },
                onFailure = { e ->
                    error = e.message
                    isLoading = false
                }
            )
        }
    }

    LaunchedEffect(Unit) { loadData() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("WiFi Networks") },
                actions = {
                    IconButton(onClick = { loadData() }) {
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
                    CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
                }
                error != null -> {
                    ErrorContent(message = error!!, modifier = Modifier.align(Alignment.Center))
                }
                credentials.isEmpty() -> {
                    EmptyContent(
                        icon = Icons.Default.Wifi,
                        title = "No credentials found",
                        subtitle = "Sync with a server to get credentials",
                        modifier = Modifier.align(Alignment.Center)
                    )
                }
                else -> {
                    LazyColumn(
                        contentPadding = PaddingValues(16.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        items(credentials) { credential ->
                            CredentialCard(credential = credential)
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun CredentialCard(credential: CredentialSummary) {
    Card(modifier = Modifier.fillMaxWidth()) {
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

// ============================================================================
// Sync Screen
// ============================================================================

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SyncScreen() {
    var syncStatus by remember { mutableStateOf<SyncStatus?>(null) }
    var isLoading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var showLoginDialog by remember { mutableStateOf(false) }
    var showPasswordDialog by remember { mutableStateOf(false) }
    var syncAction by remember { mutableStateOf<String?>(null) }
    var actionResult by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    fun loadStatus() {
        scope.launch {
            isLoading = true
            error = null
            withContext(Dispatchers.IO) {
                WifisyncCore.syncStatus()
            }.fold(
                onSuccess = { status ->
                    syncStatus = status
                    isLoading = false
                },
                onFailure = { e ->
                    error = e.message
                    isLoading = false
                }
            )
        }
    }

    LaunchedEffect(Unit) { loadStatus() }

    if (showLoginDialog) {
        LoginDialog(
            onDismiss = { showLoginDialog = false },
            onLogin = { serverUrl, username, password ->
                showLoginDialog = false
                scope.launch {
                    isLoading = true
                    withContext(Dispatchers.IO) {
                        WifisyncCore.syncLogin(serverUrl, username, password)
                    }.fold(
                        onSuccess = {
                            actionResult = "Logged in successfully"
                            loadStatus()
                        },
                        onFailure = { e ->
                            error = e.message
                            isLoading = false
                        }
                    )
                }
            }
        )
    }

    if (showPasswordDialog && syncAction != null) {
        PasswordDialog(
            title = if (syncAction == "push") "Push Changes" else "Pull Changes",
            onDismiss = {
                showPasswordDialog = false
                syncAction = null
            },
            onConfirm = { password ->
                showPasswordDialog = false
                val action = syncAction
                syncAction = null
                scope.launch {
                    isLoading = true
                    withContext(Dispatchers.IO) {
                        if (action == "push") {
                            WifisyncCore.syncPush(password)
                        } else {
                            WifisyncCore.syncPull(password)
                        }
                    }.fold(
                        onSuccess = { result ->
                            actionResult = when (action) {
                                "push" -> {
                                    val r = result as SyncPushResponse
                                    "Pushed: ${r.accepted} accepted, ${r.conflicts} conflicts"
                                }
                                else -> {
                                    val r = result as SyncPullResponse
                                    val details = r.error_details?.distinct()?.joinToString(", ") ?: ""
                                    if (r.errors > 0 && details.isNotEmpty()) {
                                        "Pulled: ${r.applied} applied, ${r.errors} errors ($details)"
                                    } else {
                                        "Pulled: ${r.applied} applied, ${r.errors} errors"
                                    }
                                }
                            }
                            loadStatus()
                        },
                        onFailure = { e ->
                            error = e.message
                            isLoading = false
                        }
                    )
                }
            }
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Sync") },
                actions = {
                    IconButton(onClick = { loadStatus() }) {
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
                    CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
                }
                error != null -> {
                    Column(
                        modifier = Modifier
                            .align(Alignment.Center)
                            .padding(16.dp),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        ErrorContent(message = error!!)
                        Spacer(modifier = Modifier.height(16.dp))
                        Button(onClick = { loadStatus() }) {
                            Text("Retry")
                        }
                    }
                }
                syncStatus != null -> {
                    LazyColumn(
                        contentPadding = PaddingValues(16.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp)
                    ) {
                        // Status message
                        item {
                            if (actionResult != null) {
                                Card(
                                    colors = CardDefaults.cardColors(
                                        containerColor = MaterialTheme.colorScheme.primaryContainer
                                    )
                                ) {
                                    Row(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .padding(16.dp),
                                        verticalAlignment = Alignment.CenterVertically
                                    ) {
                                        Icon(Icons.Default.CheckCircle, contentDescription = null)
                                        Spacer(modifier = Modifier.width(8.dp))
                                        Text(actionResult!!)
                                        Spacer(modifier = Modifier.weight(1f))
                                        IconButton(onClick = { actionResult = null }) {
                                            Icon(Icons.Default.Close, contentDescription = "Dismiss")
                                        }
                                    }
                                }
                            }
                        }

                        // Sync status card
                        item {
                            SyncStatusCard(
                                status = syncStatus!!,
                                onLogin = { showLoginDialog = true },
                                onLogout = {
                                    scope.launch {
                                        isLoading = true
                                        withContext(Dispatchers.IO) {
                                            WifisyncCore.syncLogout()
                                        }.fold(
                                            onSuccess = {
                                                actionResult = "Logged out"
                                                loadStatus()
                                            },
                                            onFailure = { e ->
                                                error = e.message
                                                isLoading = false
                                            }
                                        )
                                    }
                                }
                            )
                        }

                        // Sync actions (only if logged in)
                        if (syncStatus!!.enabled) {
                            item {
                                SyncActionsCard(
                                    pendingChanges = syncStatus!!.pendingChanges,
                                    onPush = {
                                        syncAction = "push"
                                        showPasswordDialog = true
                                    },
                                    onPull = {
                                        syncAction = "pull"
                                        showPasswordDialog = true
                                    }
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun SyncStatusCard(
    status: SyncStatus,
    onLogin: () -> Unit,
    onLogout: () -> Unit
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp)
        ) {
            Text(
                text = "Sync Status",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold
            )
            Spacer(modifier = Modifier.height(16.dp))

            if (status.enabled) {
                StatusRow("Server", status.serverUrl ?: "Unknown")
                StatusRow("Username", status.username ?: "Unknown")
                StatusRow("Device ID", status.deviceId?.let { it.take(8) + "..." } ?: "Unknown")
                StatusRow("Last Sync", status.lastSync ?: "Never")
                StatusRow("Pending Changes", status.pendingChanges.toString())
                StatusRow(
                    "Token",
                    if (status.hasValidToken) "Valid" else "Expired",
                    valueColor = if (status.hasValidToken)
                        MaterialTheme.colorScheme.primary
                    else
                        MaterialTheme.colorScheme.error
                )

                Spacer(modifier = Modifier.height(16.dp))
                OutlinedButton(
                    onClick = onLogout,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Default.Logout, contentDescription = null)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Logout")
                }
            } else {
                Text(
                    text = "Not connected to a sync server",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(16.dp))
                Button(
                    onClick = onLogin,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Default.Login, contentDescription = null)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Login to Server")
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SyncActionsCard(
    pendingChanges: Int,
    onPush: () -> Unit,
    onPull: () -> Unit
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp)
        ) {
            Text(
                text = "Sync Actions",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold
            )
            Spacer(modifier = Modifier.height(16.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Button(
                    onClick = onPush,
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(Icons.Default.CloudUpload, contentDescription = null)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Push")
                    if (pendingChanges > 0) {
                        Spacer(modifier = Modifier.width(4.dp))
                        Badge { Text(pendingChanges.toString()) }
                    }
                }

                Button(
                    onClick = onPull,
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(Icons.Default.CloudDownload, contentDescription = null)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Pull")
                }
            }
        }
    }
}

@Composable
fun StatusRow(
    label: String,
    value: String,
    valueColor: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onSurface
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodyMedium,
            color = valueColor
        )
    }
}

// ============================================================================
// Settings Screen
// ============================================================================

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen() {
    var devices by remember { mutableStateOf<List<DeviceInfo>>(emptyList()) }
    var isLoading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    fun loadData() {
        scope.launch {
            isLoading = true
            error = null
            withContext(Dispatchers.IO) {
                WifisyncCore.listDevices()
            }.fold(
                onSuccess = { devs ->
                    devices = devs
                    isLoading = false
                },
                onFailure = { _ ->
                    // If not logged in, just show empty devices list
                    devices = emptyList()
                    isLoading = false
                }
            )
        }
    }

    LaunchedEffect(Unit) { loadData() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") }
            )
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Version info
            item {
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp)
                    ) {
                        Text(
                            text = "About",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                        StatusRow("App Version", BuildConfig.VERSION_NAME)
                        StatusRow("Package", BuildConfig.APPLICATION_ID)
                    }
                }
            }

            // Devices list
            item {
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp)
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text(
                                text = "Logged In Devices",
                                style = MaterialTheme.typography.titleMedium,
                                fontWeight = FontWeight.Bold
                            )
                            IconButton(onClick = { loadData() }) {
                                Icon(Icons.Default.Refresh, contentDescription = "Refresh")
                            }
                        }

                        if (isLoading) {
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(100.dp),
                                contentAlignment = Alignment.Center
                            ) {
                                CircularProgressIndicator()
                            }
                        } else if (devices.isEmpty()) {
                            Spacer(modifier = Modifier.height(16.dp))
                            Text(
                                text = "No devices found. Login to a sync server to see devices.",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        } else {
                            Spacer(modifier = Modifier.height(8.dp))
                            devices.forEach { device ->
                                DeviceItem(device = device)
                                if (device != devices.last()) {
                                    Divider(modifier = Modifier.padding(vertical = 8.dp))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DeviceItem(device: DeviceInfo) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            imageVector = if (device.isCurrentDevice) Icons.Default.PhoneAndroid else Icons.Default.Devices,
            contentDescription = null,
            tint = if (device.isCurrentDevice)
                MaterialTheme.colorScheme.primary
            else
                MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(32.dp)
        )
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = device.name,
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (device.isCurrentDevice) FontWeight.Bold else FontWeight.Normal
                )
                if (device.isCurrentDevice) {
                    Spacer(modifier = Modifier.width(8.dp))
                    Badge { Text("This device") }
                }
            }
            Text(
                text = "Last sync: ${device.lastSyncAt ?: "Never"}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

// ============================================================================
// Dialogs
// ============================================================================

@Composable
fun LoginDialog(
    onDismiss: () -> Unit,
    onLogin: (serverUrl: String, username: String, password: String) -> Unit
) {
    var serverUrl by remember { mutableStateOf("") }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Login to Sync Server") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = serverUrl,
                    onValueChange = { serverUrl = it },
                    label = { Text("Server URL") },
                    placeholder = { Text("https://sync.example.com") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = username,
                    onValueChange = { username = it },
                    label = { Text("Username") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it },
                    label = { Text("Master Password") },
                    singleLine = true,
                    visualTransformation = if (showPassword)
                        VisualTransformation.None
                    else
                        PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    trailingIcon = {
                        IconButton(onClick = { showPassword = !showPassword }) {
                            Icon(
                                if (showPassword) Icons.Default.VisibilityOff else Icons.Default.Visibility,
                                contentDescription = if (showPassword) "Hide password" else "Show password"
                            )
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            Button(
                onClick = { onLogin(serverUrl, username, password) },
                enabled = serverUrl.isNotBlank() && username.isNotBlank() && password.isNotBlank()
            ) {
                Text("Login")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        }
    )
}

@Composable
fun PasswordDialog(
    title: String,
    onDismiss: () -> Unit,
    onConfirm: (password: String) -> Unit
) {
    var password by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column {
                Text("Enter your master password to encrypt/decrypt credentials.")
                Spacer(modifier = Modifier.height(16.dp))
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it },
                    label = { Text("Master Password") },
                    singleLine = true,
                    visualTransformation = if (showPassword)
                        VisualTransformation.None
                    else
                        PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    trailingIcon = {
                        IconButton(onClick = { showPassword = !showPassword }) {
                            Icon(
                                if (showPassword) Icons.Default.VisibilityOff else Icons.Default.Visibility,
                                contentDescription = if (showPassword) "Hide password" else "Show password"
                            )
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            Button(
                onClick = { onConfirm(password) },
                enabled = password.isNotBlank()
            ) {
                Text("Confirm")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        }
    )
}

// ============================================================================
// Common Components
// ============================================================================

@Composable
fun EmptyContent(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    subtitle: String,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium
        )
        Text(
            text = subtitle,
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

// ============================================================================
// Utilities
// ============================================================================

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
