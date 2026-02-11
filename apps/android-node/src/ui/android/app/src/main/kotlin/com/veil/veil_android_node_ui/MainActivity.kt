package com.veil.veil_android_node_ui

import android.content.Intent
import androidx.core.content.ContextCompat
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.File

class MainActivity : FlutterActivity() {
    private val channel = "veil/node_service"

    override fun onStart() {
        super.onStart()
        startNodeService()
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, channel)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "start" -> {
                        startNodeService()
                        result.success(statusMap())
                    }
                    "stop" -> {
                        stopNodeService()
                        result.success(statusMap())
                    }
                    "status" -> {
                        result.success(statusMap())
                    }
                    else -> result.notImplemented()
                }
            }
    }

    private fun startNodeService() {
        val intent = Intent(this, NodeForegroundService::class.java).apply {
            action = NodeForegroundService.ACTION_START
        }
        ContextCompat.startForegroundService(this, intent)
    }

    private fun stopNodeService() {
        val intent = Intent(this, NodeForegroundService::class.java).apply {
            action = NodeForegroundService.ACTION_STOP
        }
        startService(intent)
    }

    private fun statusMap(): Map<String, Any?> {
        return mapOf(
            "running" to NodeForegroundService.isRunning(),
            "error" to NodeForegroundService.lastError(),
            "token" to readToken(),
        )
    }

    private fun readToken(): String? {
        val token = NodeForegroundService.authToken()
        if (!token.isNullOrBlank()) {
            return token
        }
        val tokenFile = File(filesDir, "node_token.txt")
        if (!tokenFile.exists()) {
            return null
        }
        return tokenFile.readText().trim().ifEmpty { null }
    }
}
