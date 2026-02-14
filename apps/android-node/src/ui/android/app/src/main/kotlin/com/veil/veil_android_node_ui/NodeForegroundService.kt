package com.veil.veil_android_node_ui

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Log
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import java.util.UUID
import java.util.concurrent.atomic.AtomicBoolean

class NodeForegroundService : Service() {
    companion object {
        const val ACTION_START = "com.veil.veil_android_node_ui.START"
        const val ACTION_STOP = "com.veil.veil_android_node_ui.STOP"
        private const val CHANNEL_ID = "veil_node"
        private const val NOTIFICATION_ID = 1001
        private const val TAG = "VeilNodeService"
        private const val STATE_KEY_ALIAS = "veil_node_state_wrap_key"
        private const val STATE_KEY_FILE = "node_state_key.enc"

        @Volatile
        private var running: Boolean = false

        @Volatile
        private var lastError: String? = null

        @Volatile
        private var authToken: String? = null

        private val starting = AtomicBoolean(false)

        fun isRunning(): Boolean = running
        fun lastError(): String? = lastError
        fun authToken(): String? = authToken
    }

    private var process: Process? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopNodeProcess()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
                running = false
                return START_NOT_STICKY
            }
            ACTION_START, null -> {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                    startForeground(NOTIFICATION_ID, buildNotification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
                } else {
                    startForeground(NOTIFICATION_ID, buildNotification())
                }
                if (starting.compareAndSet(false, true)) {
                    Thread {
                        startNodeProcess()
                        starting.set(false)
                    }.start()
                }
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        stopNodeProcess()
        running = false
        super.onDestroy()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        Log.i(TAG, "App swiped away, stopping node service and exiting process")
        stopNodeProcess()
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
        running = false
        // Definitive exit to ensure debug connections are closed
        android.os.Process.killProcess(android.os.Process.myPid())
    }

    private fun startNodeProcess() {
        if (process != null) {
            running = true
            lastError = null
            return
        }
        try {
            val binary = ensureBinary()
            if (!binary.setExecutable(true, false)) {
                Log.w(TAG, "setExecutable failed for ${binary.absolutePath}")
            }
            // Small delay to ensure service is fully ready
            Thread.sleep(1000)
            
            val builder = ProcessBuilder(binary.absolutePath)
            builder.directory(binary.parentFile)
            builder.redirectErrorStream(true)
            val env = builder.environment()
            val token = loadOrCreateToken()
            val stateKeyHex = loadOrCreateStateKeyHex()
            env["VEIL_NODE_HOST"] = "127.0.0.1"
            env["VEIL_NODE_PORT"] = "7788"
            env["VEIL_NODE_LOG_LEVEL"] = "debug"
            env["VEIL_NODE_STATE"] = File(filesDir, "node_state.json").absolutePath
            env["VEIL_NODE_CACHE_STATE"] = File(filesDir, "node_cache.cbor").absolutePath
            env["VEIL_NODE_QUIC_BIND"] = "0.0.0.0:9000"
            env["VEIL_NODE_QUIC_SERVER_NAME"] = "localhost"
            env["VEIL_NODE_QUIC_PUBLIC"] = "127.0.0.1:9000"
            env["VEIL_NODE_WS"] = "wss://veilnode.3nostr.com/ws"
            env["VEIL_NODE_WS_PEERS"] = "wss://veilnode.3nostr.com/ws,ws://veilnode.3nostr.com/ws"
            env["VEIL_NODE_FAST_PEERS"] = "quic://veilnode.3nostr.com:5000"
            env["VEIL_NODE_TOKEN"] = token
            env["VEIL_NODE_STATE_KEY_HEX"] = stateKeyHex
            env["VEIL_DISCOVERY_BOOTSTRAP"] = "quic://veilnode.3nostr.com:5000,wss://veilnode.3nostr.com/ws"
            env["VEIL_LAN_DISCOVERY"] = "1"
            process = builder.start()
            running = true
            lastError = null
            streamNodeLogs(process!!)
            Log.i(TAG, "Node started: ${binary.absolutePath}")
        } catch (err: Exception) {
            lastError = err.message ?: "failed to start"
            running = false
            Log.e(TAG, "Node start failed", err)
        }
    }

    private fun stopNodeProcess() {
        try {
            process?.destroy()
            process?.waitFor()
        } catch (err: Exception) {
            Log.w(TAG, "Node stop failed", err)
        }
        process?.destroy()
        process = null
        running = false
    }

    private fun streamNodeLogs(proc: Process) {
        Thread {
            try {
                proc.inputStream.bufferedReader().useLines { lines ->
                    lines.forEach { line ->
                        Log.i(TAG, line)
                    }
                }
            } catch (err: Exception) {
                Log.w(TAG, "Log stream stopped", err)
            } finally {
                try {
                    val exitCode = proc.exitValue()
                    Log.i(TAG, "Node process exited with code $exitCode")
                    if (exitCode != 0) {
                        lastError = "Process exited with code $exitCode"
                    }
                } catch (e: IllegalThreadStateException) {
                    // Still running
                }
                if (process == proc) {
                    running = false
                    process = null
                }
            }
        }.start()
    }

    private fun ensureBinary(): File {
        // We MUST bundle binaries as "native libraries" (.so files) so they are extracted to nativeLibraryDir.
        // Modern Android (API 29+) forbids execution from any writable directory.
        Log.i(TAG, "Native library dir: ${applicationInfo.nativeLibraryDir}")
        val nativeBin = File(applicationInfo.nativeLibraryDir, "libveil_node.so")
        if (nativeBin.exists()) {
            Log.i(TAG, "Using native binary: ${nativeBin.absolutePath}")
            return nativeBin
        }

        // Detailed error reporting if the binary is missing from the APK
        val abi = Build.SUPPORTED_ABIS?.firstOrNull() ?: "unknown"
        throw IllegalStateException("Binary 'libveil_node.so' not found in nativeLibraryDir for ABI $abi. " +
                "Check that src/main/jniLibs contains the correct architecture folders and .so files, " +
                "and that you have done a FULL REINSTALL (uninstall first).")
    }

    private fun loadOrCreateToken(): String {
        val tokenFile = File(filesDir, "node_token.txt")
        val existing = if (tokenFile.exists()) {
            tokenFile.readText().trim()
        } else {
            ""
        }
        if (existing.isNotEmpty()) {
            authToken = existing
            return existing
        }
        val created = UUID.randomUUID().toString()
        tokenFile.writeText(created)
        authToken = created
        return created
    }

    private fun loadOrCreateStateKeyHex(): String {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            val legacyFile = File(filesDir, "node_state_key_hex.txt")
            val legacy = legacyFile.takeIf { it.exists() }?.readText()?.trim()
            val keyHex = if (!legacy.isNullOrBlank() && legacy.length == 64) {
                legacy
            } else {
                val value = ByteArray(32)
                java.security.SecureRandom().nextBytes(value)
                value.joinToString(separator = "") { byte ->
                    "%02x".format(byte.toInt() and 0xFF)
                }.also { legacyFile.writeText(it) }
            }
            return keyHex
        }
        val keyFile = File(filesDir, STATE_KEY_FILE)
        val wrapped = keyFile.takeIf { it.exists() }?.readText()?.trim()
        val stateKeyBytes = if (!wrapped.isNullOrBlank()) {
            decryptStateKey(wrapped) ?: createAndPersistStateKey(keyFile)
        } else {
            createAndPersistStateKey(keyFile)
        }
        return stateKeyBytes.joinToString(separator = "") { byte ->
            "%02x".format(byte.toInt() and 0xFF)
        }
    }

    private fun createAndPersistStateKey(target: File): ByteArray {
        val value = ByteArray(32)
        java.security.SecureRandom().nextBytes(value)
        val wrapped = encryptStateKey(value)
        target.writeText(wrapped)
        return value
    }

    private fun encryptStateKey(raw: ByteArray): String {
        val key = getOrCreateWrappingKey()
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key)
        val ciphertext = cipher.doFinal(raw)
        val ivB64 = android.util.Base64.encodeToString(cipher.iv, android.util.Base64.NO_WRAP)
        val ctB64 =
            android.util.Base64.encodeToString(ciphertext, android.util.Base64.NO_WRAP)
        return "$ivB64:$ctB64"
    }

    private fun decryptStateKey(encoded: String): ByteArray? {
        val parts = encoded.split(':')
        if (parts.size != 2) {
            return null
        }
        return try {
            val iv = android.util.Base64.decode(parts[0], android.util.Base64.NO_WRAP)
            val ciphertext = android.util.Base64.decode(parts[1], android.util.Base64.NO_WRAP)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            val spec = GCMParameterSpec(128, iv)
            cipher.init(Cipher.DECRYPT_MODE, getOrCreateWrappingKey(), spec)
            cipher.doFinal(ciphertext)
        } catch (err: Exception) {
            Log.w(TAG, "Failed to decrypt state key, rotating", err)
            null
        }
    }

    private fun getOrCreateWrappingKey(): SecretKey {
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (keyStore.getKey(STATE_KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val keyGenerator =
            KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        val spec = KeyGenParameterSpec.Builder(
            STATE_KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .build()
        keyGenerator.init(spec)
        return keyGenerator.generateKey()
    }

    private fun selectAssetName(): String {
        val abiList = Build.SUPPORTED_ABIS ?: emptyArray()
        for (abi in abiList) {
            val candidate = "veil_node_$abi"
            if (assetExists(candidate)) {
                return candidate
            }
        }
        if (assetExists("veil_node")) {
            return "veil_node"
        }
        throw IllegalStateException("No embedded node binary found for ABI")
    }

    private fun assetExists(name: String): Boolean {
        return try {
            assets.open(name).close()
            true
        } catch (_: Exception) {
            false
        }
    }

    private fun buildNotification(): Notification {
        ensureChannel()
        
        val stopIntent = Intent(this, NodeForegroundService::class.java).apply {
            action = ACTION_STOP
        }
        val stopPendingIntent = PendingIntent.getService(
            this, 0, stopIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }

        return builder
            .setContentTitle("Veil Node")
            .setContentText("Node service running")
            .setSmallIcon(R.mipmap.ic_launcher)
            .setOngoing(true)
            .addAction(
                Notification.Action.Builder(
                    null, "Stop", stopPendingIntent
                ).build()
            )
            .build()
    }

    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return
        }
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val channel = NotificationChannel(
            CHANNEL_ID,
            "Veil Node",
            NotificationManager.IMPORTANCE_LOW
        )
        manager.createNotificationChannel(channel)
    }
}
