package com.veil.veil_android_node_ui

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import android.util.Log
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import java.util.concurrent.atomic.AtomicBoolean

class NodeForegroundService : Service() {
    companion object {
        const val ACTION_START = "com.veil.veil_android_node_ui.START"
        const val ACTION_STOP = "com.veil.veil_android_node_ui.STOP"
        private const val CHANNEL_ID = "veil_node"
        private const val NOTIFICATION_ID = 1001
        private const val TAG = "VeilNodeService"

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
                startForeground(NOTIFICATION_ID, buildNotification())
                if (starting.compareAndSet(false, true)) {
                    Thread {
                        startNodeProcess()
                        starting.set(false)
                    }.start()
                }
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        stopNodeProcess()
        running = false
        super.onDestroy()
    }

    private fun startNodeProcess() {
        if (process != null) {
            running = true
            lastError = null
            return
        }
        try {
            val binary = ensureBinary()
            binary.setExecutable(true, false)
            val builder = ProcessBuilder(binary.absolutePath)
            builder.directory(filesDir)
            builder.redirectErrorStream(true)
            val env = builder.environment()
            val token = loadOrCreateToken()
            env["VEIL_NODE_PORT"] = "7788"
            env["VEIL_NODE_STATE"] = File(filesDir, "node_state.json").absolutePath
            env["VEIL_NODE_CACHE_STATE"] = File(filesDir, "node_cache.cbor").absolutePath
            env["VEIL_NODE_QUIC_BIND"] = "0.0.0.0:9000"
            env["VEIL_NODE_QUIC_SERVER_NAME"] = "localhost"
            env["VEIL_NODE_TOKEN"] = token
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
            }
        }.start()
    }

    private fun ensureBinary(): File {
        val target = File(filesDir, "veil_node")
        if (target.exists()) {
            return target
        }
        val assetName = selectAssetName()
        assets.open(assetName).use { input ->
            FileOutputStream(target).use { output ->
                input.copyTo(output)
            }
        }
        return target
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
        return Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("Veil Node")
            .setContentText("Node service running")
            .setSmallIcon(R.mipmap.ic_launcher)
            .setOngoing(true)
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
