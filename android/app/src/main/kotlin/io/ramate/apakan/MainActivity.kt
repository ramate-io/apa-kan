package io.ramate.apakan

import android.content.Intent
import android.os.Bundle
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.google.androidgamesdk.GameActivity

/**
 * Thin native wrapper. Bevy owns every game screen.
 *
 * Forwards `apakan://` and https App Links into the Rust cdylib as an
 * `IncomingUrl` message. External https pages (payments, legal) leave the
 * process via `ACTION_VIEW` from Rust (`webbrowser` crate) — no Custom Tabs,
 * Compose, or in-app browser.
 */
class MainActivity : GameActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        installSplashScreen()
        super.onCreate(savedInstanceState)
        WindowCompat.enableEdgeToEdge(window)
        forwardIncomingUrl(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        forwardIncomingUrl(intent)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            hideSystemUi()
        }
    }

    private fun hideSystemUi() {
        window.decorView.post {
            val controller = WindowCompat.getInsetsController(window, window.decorView)
            controller.systemBarsBehavior =
                WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            controller.hide(WindowInsetsCompat.Type.systemBars())
        }
    }

    private fun forwardIncomingUrl(intent: Intent?) {
        val uri = intent?.data?.toString() ?: return
        if (uri.isEmpty()) return
        nativeOnIncomingUrl(uri)
    }

    private external fun nativeOnIncomingUrl(uri: String)

    companion object {
        init {
            System.loadLibrary("apa_kan")
        }
    }
}
