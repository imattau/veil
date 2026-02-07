package app.veil.android

import android.content.pm.PackageManager
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import java.net.InetSocketAddress
import java.net.Proxy
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.atomic.AtomicInteger

class MainActivity : FlutterActivity() {
  private val torChannel = "veil_tor"
  private val torConnections = ConcurrentHashMap<Int, TorConnection>()
  private val nextTorId = AtomicInteger(1)

  override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
    super.configureFlutterEngine(flutterEngine)
    MethodChannel(flutterEngine.dartExecutor.binaryMessenger, torChannel)
      .setMethodCallHandler { call, result ->
        when (call.method) {
          "isSupported" -> result.success(isOrbotInstalled())
          "connect" -> {
            val url = call.argument<String>("url") ?: ""
            val socksHost = call.argument<String>("socksHost") ?: "127.0.0.1"
            val socksPort = call.argument<Int>("socksPort") ?: 9050
            if (url.isBlank()) {
              result.error("INVALID", "url required", null)
              return@setMethodCallHandler
            }
            val id = connectTor(url, socksHost, socksPort)
            result.success(id)
          }
          "send" -> {
            val id = call.argument<Int>("id") ?: 0
            val bytes = call.argument<ByteArray>("bytes")
            val conn = torConnections[id]
            if (conn == null || bytes == null) {
              result.error("INVALID", "connection not found", null)
              return@setMethodCallHandler
            }
            conn.socket?.send(ByteString.of(*bytes))
            result.success(null)
          }
          "recv" -> {
            val id = call.argument<Int>("id") ?: 0
            val conn = torConnections[id]
            if (conn == null) {
              result.success(null)
              return@setMethodCallHandler
            }
            val bytes = conn.inbox.poll()
            if (bytes == null) {
              result.success(null)
            } else {
              result.success(mapOf("peer" to "tor", "bytes" to bytes))
            }
          }
          "close" -> {
            val id = call.argument<Int>("id") ?: 0
            torConnections.remove(id)?.close()
            result.success(null)
          }
          else -> result.notImplemented()
        }
      }
  }

  private fun connectTor(url: String, socksHost: String, socksPort: Int): Int {
    val proxy = Proxy(Proxy.Type.SOCKS, InetSocketAddress(socksHost, socksPort))
    val client = OkHttpClient.Builder().proxy(proxy).build()
    val request = Request.Builder().url(url).build()
    val connection = TorConnection()
    val socket = client.newWebSocket(
      request,
      object : WebSocketListener() {
        override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
          connection.inbox.add(bytes.toByteArray())
        }
      },
    )
    connection.socket = socket
    val id = nextTorId.getAndIncrement()
    torConnections[id] = connection
    return id
  }

  private fun isOrbotInstalled(): Boolean {
    return try {
      packageManager.getPackageInfo("org.torproject.android", PackageManager.PackageInfoFlags.of(0))
      true
    } catch (_: Exception) {
      false
    }
  }

  private class TorConnection {
    val inbox = ConcurrentLinkedQueue<ByteArray>()
    var socket: WebSocket? = null
    fun close() {
      socket?.close(1000, "closed")
    }
  }
}
