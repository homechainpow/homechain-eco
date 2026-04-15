package com.homechain.miner

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

import android.content.Intent
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Robust Solution: Start the persistent foreground service on launch
    val intent = Intent(this, MiningForegroundService::class.java)
    ContextCompat.startForegroundService(this, intent)
  }

  override fun onDestroy() {
    super.onDestroy()
    // DEATH-ON-SWIPE: Stop service and kill process to guarantee no zombie mining
    val intent = Intent(this, MiningForegroundService::class.java)
    stopService(intent)
    android.os.Process.killProcess(android.os.Process.myPid())
  }
}
