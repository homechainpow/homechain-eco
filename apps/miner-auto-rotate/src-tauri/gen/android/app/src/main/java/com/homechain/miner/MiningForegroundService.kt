package com.homechain.miner

import android.app.*
import android.content.Intent
import android.os.*
import androidx.core.app.NotificationCompat

class MiningForegroundService : Service() {
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // 1. Create Notification Channel (required for Android 8+)
        val channel = NotificationChannel(
            "mining_channel", "Mining Service",
            NotificationManager.IMPORTANCE_LOW
        )
        val notificationManager = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.createNotificationChannel(channel)

        // 2. Build Persistent Notification
        val notification = NotificationCompat.Builder(this, "mining_channel")
            .setContentTitle("HomeChain Miner")
            .setContentText("⛏️ Mining running in background...")
            // We use standard Android icon as Tauri doesn't copy ic_launcher to standard R folder sometimes easily
            .setSmallIcon(android.R.drawable.ic_menu_manage)
            .setOngoing(true)
            .build()

        // 3. Promote to Foreground Service
        startForeground(1, notification)

        // 4. Acquire WakeLock
        val pm = getSystemService(POWER_SERVICE) as PowerManager
        wakeLock = pm.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "HomeChain::MiningWakeLock"
        ).apply { acquire() }

        // Request OS to restart service if it kills it for memory
        return START_STICKY
    }

    override fun onDestroy() {
        super.onDestroy()
        if (wakeLock?.isHeld == true) {
            wakeLock?.release()
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
