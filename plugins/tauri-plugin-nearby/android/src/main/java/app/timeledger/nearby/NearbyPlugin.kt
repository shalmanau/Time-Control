package app.timeledger.nearby

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import android.content.pm.PackageManager
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray
import java.io.File
import java.util.concurrent.ConcurrentHashMap

@InvokeArg
class StartArgs { var port: Int = 0; var group: String = ""; var name: String = "Android phone" }
@InvokeArg
class InstallArgs { var path: String = ""; var versionCode: Long = 0 }

@TauriPlugin
class NearbyPlugin(private val activity: Activity): Plugin(activity) {
    private val nsd = activity.getSystemService(Context.NSD_SERVICE) as NsdManager
    private val peers = ConcurrentHashMap<String, JSObject>()
    private val removed = mutableListOf<String>()
    private var registration: NsdManager.RegistrationListener? = null
    private var discovery: NsdManager.DiscoveryListener? = null
    private var lock: WifiManager.MulticastLock? = null
    private var ownName = ""
    private var discoveryError: String? = null

    @Command
    fun wifi(invoke: Invoke) {
        val cm = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val caps = cm.getNetworkCapabilities(cm.activeNetwork)
        invoke.resolve(JSObject().put("connected", caps?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true))
    }
    @Command
    fun start(invoke: Invoke) {
        val args = invoke.parseArgs(StartArgs::class.java)
        if (args.port !in 1..65535 || args.group.isBlank()) { invoke.reject("Invalid discovery configuration"); return }
        try {
            registration?.let { try { nsd.unregisterService(it) } catch (_: Exception) {} }
            discovery?.let { try { nsd.stopServiceDiscovery(it) } catch (_: Exception) {} }
            if (lock == null) {
                lock = (activity.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager).createMulticastLock("time-ledger-discovery").apply { setReferenceCounted(false); acquire() }
            }
            val info = NsdServiceInfo().apply {
                serviceName = "TimeLedger-${args.group.take(8)}-${args.port}"
                serviceType = "_timeledger._tcp."
                port = args.port
                setAttribute("name", args.name)
                setAttribute("group", args.group)
            }
            ownName = info.serviceName
            registration = object: NsdManager.RegistrationListener {
                override fun onServiceRegistered(s: NsdServiceInfo) { ownName = s.serviceName }
                override fun onRegistrationFailed(s: NsdServiceInfo, e: Int) { discoveryError = "Network announcement failed ($e)" }
                override fun onServiceUnregistered(s: NsdServiceInfo) {}
                override fun onUnregistrationFailed(s: NsdServiceInfo, e: Int) {}
            }
            nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, registration)
            discovery = object: NsdManager.DiscoveryListener {
                override fun onDiscoveryStarted(type: String) {}
                override fun onDiscoveryStopped(type: String) {}
                override fun onStartDiscoveryFailed(type: String, error: Int) { discoveryError = "Nearby discovery failed ($error)" }
                override fun onStopDiscoveryFailed(type: String, error: Int) {}
                override fun onServiceFound(service: NsdServiceInfo) {
                    if (service.serviceName == ownName) return
                    @Suppress("DEPRECATION")
                    nsd.resolveService(service, object: NsdManager.ResolveListener {
                        override fun onResolveFailed(s: NsdServiceInfo, error: Int) { /* Discovery retries on the next announcement. */ }
                        override fun onServiceResolved(s: NsdServiceInfo) {
                            @Suppress("DEPRECATION")
                            val host = s.host?.hostAddress ?: return
                            val address = if (host.contains(":")) "[$host]:${s.port}" else "$host:${s.port}"
                            val name = s.attributes["name"]?.toString(Charsets.UTF_8) ?: s.serviceName
                            val group = s.attributes["group"]?.toString(Charsets.UTF_8) ?: return
                            peers[s.serviceName] = JSObject().put("key", s.serviceName).put("name", name).put("group_id", group).put("address", address)
                        }
                    })
                }
                override fun onServiceLost(service: NsdServiceInfo) { peers.remove(service.serviceName); synchronized(removed) { removed.add(service.serviceName) } }
            }
            nsd.discoverServices("_timeledger._tcp.", NsdManager.PROTOCOL_DNS_SD, discovery)
            invoke.resolve()
        } catch (e: Exception) { invoke.reject(e.message ?: "Nearby discovery unavailable") }
    }
    @Command
    fun poll(invoke: Invoke) {
        discoveryError?.let { discoveryError = null; invoke.reject(it); return }
        val lost = synchronized(removed) { JSONArray(removed.toList()).also { removed.clear() } }
        invoke.resolve(JSObject().put("peers", JSONArray(peers.values.toList())).put("removed", lost))
    }
    @Command
    fun install(invoke: Invoke) {
        val args = invoke.parseArgs(InstallArgs::class.java)
        try {
            val file = File(args.path).canonicalFile
            val cache = activity.cacheDir.canonicalFile
            require(file.name == "update.apk" && file.path.startsWith(cache.path + File.separator) && file.isFile) { "Update is outside the private cache" }
            val pm = activity.packageManager
            @Suppress("DEPRECATION")
            val flags = if (Build.VERSION.SDK_INT >= 28) PackageManager.GET_SIGNING_CERTIFICATES else PackageManager.GET_SIGNATURES
            @Suppress("DEPRECATION")
            val archive = pm.getPackageArchiveInfo(file.path, flags) ?: error("Invalid Android package")
            @Suppress("DEPRECATION")
            val current = pm.getPackageInfo(activity.packageName, flags)
            require(archive.packageName == activity.packageName) { "Update belongs to a different app" }
            @Suppress("DEPRECATION")
            val version = if (Build.VERSION.SDK_INT >= 28) archive.longVersionCode else archive.versionCode.toLong()
            @Suppress("DEPRECATION")
            val installed = if (Build.VERSION.SDK_INT >= 28) current.longVersionCode else current.versionCode.toLong()
            require(version == args.versionCode && version > installed) { "Update version does not match the release" }
            @Suppress("DEPRECATION")
            val archiveSigners = if (Build.VERSION.SDK_INT >= 28) archive.signingInfo?.apkContentsSigners else archive.signatures
            @Suppress("DEPRECATION")
            val currentSigners = if (Build.VERSION.SDK_INT >= 28) current.signingInfo?.apkContentsSigners else current.signatures
            require(!archiveSigners.isNullOrEmpty() && !currentSigners.isNullOrEmpty() && archiveSigners.toSet() == currentSigners.toSet()) { "Update signing certificate does not match this app" }
            if (!pm.canRequestPackageInstalls()) {
                activity.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${activity.packageName}")))
                invoke.reject("Allow installs from Time Ledger, then tap Install again."); return
            }
            val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.ledgerupdates", file)
            activity.startActivity(Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION))
            invoke.resolve()
        } catch (e: Exception) { invoke.reject(e.message ?: "Could not open Android installer") }
    }
}

class LedgerUpdateProvider: FileProvider()
